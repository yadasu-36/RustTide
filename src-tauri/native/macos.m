// Narrow AppKit/Accessibility/Keychain bridge. Business logic lives in Rust.
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <Security/Security.h>
#import <CFNetwork/CFNetwork.h>
#import <NaturalLanguage/NaturalLanguage.h>

static AXUIElementRef target = NULL;
static pid_t targetPID = 0;
static NSString *sourceText = nil;
static NSString *sourceValue = nil;
static CFRange sourceRange;
static BOOL hasRange = NO, wholeField = NO, writable = NO;

static char *json(NSDictionary *value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    return strdup([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
}
static char *failure(NSString *message) { return json(@{@"error":message}); }
void rt_free(char *value) { free(value); }
void rt_clear_capture(void) {
    if (target) CFRelease(target);
    target = NULL; targetPID = 0; sourceText = nil; sourceValue = nil;
    hasRange = NO; wholeField = NO; writable = NO;
}
static id attribute(AXUIElementRef element, CFStringRef name) {
    if (!element) return nil;
    CFTypeRef value = NULL;
    if (AXUIElementCopyAttributeValue(element, name, &value) != kAXErrorSuccess) return nil;
    return CFBridgingRelease(value);
}
static NSString *stringAttribute(AXUIElementRef element, CFStringRef name) {
    id value = attribute(element, name);
    return [value isKindOfClass:NSString.class] ? value : nil;
}
static AXUIElementRef focusedElement(pid_t pid) {
    AXUIElementRef app = AXUIElementCreateApplication(pid);
    CFTypeRef value = NULL;
    AXError error = AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute, &value);
    CFRelease(app);
    if (error != kAXErrorSuccess || !value) return NULL;
    if (CFGetTypeID(value) != AXUIElementGetTypeID()) { CFRelease(value); return NULL; }
    return (AXUIElementRef)value;
}
static BOOL rangeOf(AXUIElementRef element, CFRange *range) {
    id value = attribute(element, kAXSelectedTextRangeAttribute);
    if (!value || CFGetTypeID((__bridge CFTypeRef)value) != AXValueGetTypeID()) return NO;
    return AXValueGetValue((__bridge AXValueRef)value, kAXValueCFRangeType, range);
}
static BOOL isSecure(AXUIElementRef element) {
    AXUIElementRef current = element ? (AXUIElementRef)CFRetain(element) : NULL;
    for (int i = 0; current && i < 10; i++) {
        NSString *subrole = stringAttribute(current, kAXSubroleAttribute);
        if ([subrole isEqualToString:(__bridge NSString *)kAXSecureTextFieldSubrole]) {
            CFRelease(current); return YES;
        }
        id parent = attribute(current, kAXParentAttribute);
        CFRelease(current); current = NULL;
        if (parent && CFGetTypeID((__bridge CFTypeRef)parent) == AXUIElementGetTypeID())
            current = (AXUIElementRef)CFRetain((__bridge CFTypeRef)parent);
    }
    if (current) CFRelease(current);
    return NO;
}
static BOOL editable(AXUIElementRef element) {
    if (!element || isSecure(element)) return NO;
    Boolean value = false, selected = false;
    AXUIElementIsAttributeSettable(element, kAXValueAttribute, &value);
    AXUIElementIsAttributeSettable(element, kAXSelectedTextAttribute, &selected);
    return value || selected;
}
static void settle(double seconds) {
    // Run the main loop while the target app handles native key events.
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while (end.timeIntervalSinceNow > 0)
        [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:
            [NSDate dateWithTimeIntervalSinceNow:MIN(0.01, end.timeIntervalSinceNow)]];
}
static BOOL press(CGKeyCode key) {
    CGEventRef down = CGEventCreateKeyboardEvent(NULL, key, true);
    CGEventRef up = CGEventCreateKeyboardEvent(NULL, key, false);
    if (!down || !up) { if (down) CFRelease(down); if (up) CFRelease(up); return NO; }
    CGEventSetFlags(down, kCGEventFlagMaskCommand);
    CGEventSetFlags(up, kCGEventFlagMaskCommand);
    CGEventPost(kCGHIDEventTap, down); CGEventPost(kCGHIDEventTap, up);
    CFRelease(down); CFRelease(up); return YES;
}
static NSArray<NSPasteboardItem *> *snapshot(void) {
    NSMutableArray *items = [NSMutableArray array];
    for (NSPasteboardItem *item in NSPasteboard.generalPasteboard.pasteboardItems) {
        NSPasteboardItem *copy = [NSPasteboardItem new];
        for (NSPasteboardType type in item.types) {
            NSData *data = [item dataForType:type];
            if (data) [copy setData:data forType:type];
        }
        [items addObject:copy];
    }
    return items;
}
static void restore(NSArray<NSPasteboardItem *> *saved, NSInteger expectedCount) {
    NSPasteboard *pb = NSPasteboard.generalPasteboard;
    if (pb.changeCount != expectedCount) return;
    [pb clearContents];
    if (saved.count) [pb writeObjects:saved];
}
static NSString *copySelection(void) {
    NSPasteboard *pb = NSPasteboard.generalPasteboard;
    NSArray *saved = snapshot();
    NSInteger before = pb.changeCount;
    if (!press(8)) return nil; // Command+C
    for (int i = 0; i < 40 && pb.changeCount == before; i++) settle(0.01);
    if (pb.changeCount == before) return nil;
    NSInteger copiedCount = pb.changeCount;
    NSString *copied = [pb stringForType:NSPasteboardTypeString];
    restore(saved, copiedCount);
    return copied;
}
int rt_permission(int prompt) {
    if (prompt) return AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)
        @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES});
    return AXIsProcessTrusted();
}
char *rt_capture(int allowWhole, int allowClipboard) {
    @autoreleasepool {
        rt_clear_capture();
        if (!AXIsProcessTrusted()) return failure(@"请先在系统设置 → 隐私与安全性 → 辅助功能中允许 RustTide，然后重新取词。");
        NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
        if (!front || front.processIdentifier == NSProcessInfo.processInfo.processIdentifier)
            return failure(@"请在其他应用中选中文字，再按取词快捷键。");
        targetPID = front.processIdentifier;
        target = focusedElement(targetPID);
        if (isSecure(target)) { rt_clear_capture(); return failure(@"不能读取密码输入框。"); }
        writable = editable(target);
        hasRange = rangeOf(target, &sourceRange);
        sourceValue = stringAttribute(target, kAXValueAttribute);
        sourceText = stringAttribute(target, kAXSelectedTextAttribute);
        NSString *method = @"accessibility";
        BOOL attemptedCopy = NO;
        // An AX selection can exist even when AXSelectedText is unsupported.
        // Try copy before treating that selection as a whole-field rewrite.
        if (!sourceText.length && hasRange && sourceRange.length > 0 && allowClipboard) {
            sourceText = copySelection(); method = @"clipboard";
            attemptedCopy = YES;
        }
        if (!sourceText.length && allowWhole && writable && sourceValue.length && (!hasRange || sourceRange.length == 0)) {
            sourceText = sourceValue; wholeField = YES;
        }
        if (!sourceText.length && allowClipboard && !attemptedCopy) {
            sourceText = copySelection(); method = @"clipboard";
        }
        // Refuse any capture performed while the user switched applications/fields.
        AXUIElementRef current = focusedElement(targetPID);
        BOOL same = ((!target && !current) || (target && current && CFEqual(current, target))) &&
            NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == targetPID;
        if (current) CFRelease(current);
        if (!same || !sourceText.length) {
            rt_clear_capture(); return failure(@"未取得选中文字，请保持原应用焦点并重新选择。");
        }
        BOOL replaceable = writable && (wholeField || hasRange || [method isEqualToString:@"clipboard"]);
        return json(@{@"text": sourceText, @"app": front.localizedName ?: @"应用",
            @"method": method, @"replaceable": @(replaceable), @"whole": @(wholeField)});
    }
}
char *rt_replace(const char *text) {
    @autoreleasepool {
        if (!AXIsProcessTrusted()) return failure(@"辅助功能权限已失效，请重新授权。");
        if (!target || !sourceText || !writable) return failure(@"没有可替换的输入框，请重新取词或复制译文。");
        NSString *replacement = [NSString stringWithUTF8String:text];
        if (!replacement.length) return failure(@"译文不能为空。");
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:targetPID];
        if (!app || app.terminated) return failure(@"原应用已退出，请重新取词。");
        // The nonactivating popup leaves focus in the source app. Never pull
        // the user back to a different app after a slow API response.
        if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != targetPID)
            return failure(@"你已切换到其他应用，已停止替换。请回到原输入框重新触发。");
        AXUIElementRef current = focusedElement(targetPID);
        BOOL same = current && CFEqual(current, target) &&
            NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == targetPID;
        if (current) CFRelease(current);
        if (!same) return failure(@"原输入框的焦点已改变，已停止替换。请重新取词。");
        NSString *value = stringAttribute(target, kAXValueAttribute);
        if (sourceValue && ![sourceValue isEqualToString:value])
            return failure(@"原文已发生变化，已停止替换。请重新取词。");
        CFRange now;
        if (hasRange && (!rangeOf(target, &now) || now.location != sourceRange.location || now.length != sourceRange.length))
            return failure(@"选区已发生变化，已停止替换。请重新取词。");
        NSString *selected = stringAttribute(target, kAXSelectedTextAttribute);
        if (!wholeField && !selected.length) selected = copySelection();
        if (!wholeField && ![sourceText isEqualToString:selected])
            return failure(@"当前选中文字与取词时不同，已停止替换。");
        if (wholeField) {
            CFRange all = CFRangeMake(0, (CFIndex)sourceValue.length);
            AXValueRef range = AXValueCreate(kAXValueCFRangeType, &all);
            AXError error = AXUIElementSetAttributeValue(target, kAXSelectedTextRangeAttribute, range);
            CFRelease(range);
            if (error != kAXErrorSuccess) return failure(@"无法选中整个输入框，请手动全选后重新取词。");
            settle(0.04);
        }
        // Check focus again after clipboard fallback/selection adjustment.
        current = focusedElement(targetPID);
        same = current && CFEqual(current, target) &&
            NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == targetPID;
        if (current) CFRelease(current);
        if (!same) return failure(@"焦点改变，已停止粘贴。");
        NSArray *saved = snapshot();
        NSPasteboard *pb = NSPasteboard.generalPasteboard;
        [pb clearContents];
        [pb setString:replacement forType:NSPasteboardTypeString];
        NSInteger pasteCount = pb.changeCount;
        BOOL sent = press(9); // Command+V: use native undo stack.
        settle(0.25);
        restore(saved, pasteCount);
        if (!sent) return failure(@"无法发送粘贴，请复制译文后手动粘贴。");
        NSString *expected = nil;
        if (sourceValue && (wholeField || hasRange)) {
            NSRange range = wholeField ? NSMakeRange(0, sourceValue.length) : NSMakeRange(sourceRange.location, sourceRange.length);
            if (NSMaxRange(range) <= sourceValue.length)
                expected = [sourceValue stringByReplacingCharactersInRange:range withString:replacement];
        }
        BOOL verified = expected && [expected isEqualToString:stringAttribute(target, kAXValueAttribute)];
        rt_clear_capture();
        return json(@{@"verified": @(verified)});
    }
}
char *rt_copy(const char *text) {
    @autoreleasepool {
        NSPasteboard *pb = NSPasteboard.generalPasteboard;
        [pb clearContents];
        BOOL ok = [pb setString:[NSString stringWithUTF8String:text] forType:NSPasteboardTypeString];
        return ok ? json(@{@"ok":@YES}) : failure(@"无法写入剪贴板。");
    }
}
static NSMutableDictionary *keyQuery(const char *account) {
    return [@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService:@"RustTide API",
        (__bridge id)kSecAttrAccount:[NSString stringWithUTF8String:account]} mutableCopy];
}
char *rt_key_get(const char *account) {
    @autoreleasepool {
        NSMutableDictionary *query = keyQuery(account);
        query[(__bridge id)kSecReturnData] = @YES;
        CFTypeRef data = NULL;
        OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &data);
        if (status == errSecItemNotFound) return json(@{@"value":@""});
        if (status != errSecSuccess) return failure(@"读取钥匙串失败，请解锁登录钥匙串后重试。");
        NSString *value = [[NSString alloc] initWithData:CFBridgingRelease(data) encoding:NSUTF8StringEncoding];
        return json(@{@"value":value ?: @""});
    }
}
char *rt_key_set(const char *account, const char *key) {
    @autoreleasepool {
        NSMutableDictionary *query = keyQuery(account);
        NSData *data = [[NSString stringWithUTF8String:key] dataUsingEncoding:NSUTF8StringEncoding];
        NSDictionary *attrs = @{(__bridge id)kSecValueData:data};
        OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)attrs);
        if (status == errSecItemNotFound) {
            query[(__bridge id)kSecValueData] = data;
            status = SecItemAdd((__bridge CFDictionaryRef)query, NULL);
        }
        return status == errSecSuccess ? json(@{@"ok":@YES}) : failure(@"保存钥匙串失败。");
    }
}
char *rt_key_delete(const char *account) {
    @autoreleasepool {
        OSStatus status = SecItemDelete((__bridge CFDictionaryRef)keyQuery(account));
        return status == errSecSuccess || status == errSecItemNotFound ? json(@{@"ok":@YES}) : failure(@"删除钥匙串失败。");
    }
}
char *rt_system_proxy(void) {
    @autoreleasepool {
        NSDictionary *settings = CFBridgingRelease(CFNetworkCopySystemProxySettings());
        if ([settings[(__bridge NSString *)kCFNetworkProxiesHTTPSEnable] boolValue]) {
            NSString *host = settings[(__bridge NSString *)kCFNetworkProxiesHTTPSProxy];
            NSNumber *port = settings[(__bridge NSString *)kCFNetworkProxiesHTTPSPort];
            if (host.length && port.intValue > 0)
                return json(@{@"url":[NSString stringWithFormat:@"http://%@:%@", host, port]});
        }
        return json(@{});
    }
}

char *rt_frontmost_bundle(void) {
    @autoreleasepool {
        return json(@{@"bundle":NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier ?: @""});
    }
}
char *rt_detect_language(const char *text) {
    @autoreleasepool {
        NLLanguageRecognizer *recognizer = [NLLanguageRecognizer new];
        [recognizer processString:[NSString stringWithUTF8String:text]];
        return json(@{@"language":recognizer.dominantLanguage ?: @""});
    }
}

// AX uses a top-left origin, Cocoa a bottom-left origin. Flip against the
// primary display, not maxY across displays, so vertically stacked screens work.
NSRect rt_capture_anchor(void) {
    CGRect rect = CGRectZero;
    BOOL found = NO;
    if (target && hasRange) {
        AXValueRef range = AXValueCreate(kAXValueCFRangeType, &sourceRange);
        CFTypeRef value = NULL;
        if (AXUIElementCopyParameterizedAttributeValue(target, kAXBoundsForRangeParameterizedAttribute, range, &value) == kAXErrorSuccess && value) {
            if (CFGetTypeID(value) == AXValueGetTypeID())
                found = AXValueGetValue((AXValueRef)value, kAXValueCGRectType, &rect) && rect.size.height > 0;
            CFRelease(value);
        }
        CFRelease(range);
    }
    if (!found && target) {
        id position = attribute(target, kAXPositionAttribute);
        id size = attribute(target, kAXSizeAttribute);
        if (position && size && CFGetTypeID((__bridge CFTypeRef)position) == AXValueGetTypeID() && CFGetTypeID((__bridge CFTypeRef)size) == AXValueGetTypeID()) {
            found = AXValueGetValue((__bridge AXValueRef)position, kAXValueCGPointType, &rect.origin) &&
                AXValueGetValue((__bridge AXValueRef)size, kAXValueCGSizeType, &rect.size) && rect.size.height > 0;
        }
    }
    if (found) return NSMakeRect(rect.origin.x, NSScreen.screens.firstObject.frame.size.height - CGRectGetMaxY(rect), rect.size.width, rect.size.height);
    NSPoint mouse = NSEvent.mouseLocation;
    return NSMakeRect(mouse.x, mouse.y, 1, 18);
}
