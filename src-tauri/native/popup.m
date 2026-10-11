// Transient macOS presentation only. Routing, API work and result authority
// remain in Rust. A nonactivating panel never moves typing focus on show.
#import <AppKit/AppKit.h>

extern NSRect rt_capture_anchor(void);
static void (*actionHandler)(int, uint64_t) = NULL;
static uint64_t requestID = 0;
static BOOL directMode = NO;

@interface RTPopupPanel : NSPanel
@end
@implementation RTPopupPanel
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface RTPopupActions : NSObject
- (void)close:(id)sender;
- (void)copy:(id)sender;
- (void)replace:(id)sender;
- (void)settings:(id)sender;
@end
@implementation RTPopupActions
- (void)close:(id)sender { if (actionHandler) actionHandler(1, requestID); }
- (void)copy:(id)sender { if (actionHandler) actionHandler(2, requestID); }
- (void)replace:(id)sender { if (actionHandler) actionHandler(3, requestID); }
- (void)settings:(id)sender { if (actionHandler) actionHandler(4, requestID); }
@end

static RTPopupPanel *panel;
static RTPopupActions *actions;
static NSTextField *heading, *sourceLabel, *statusLabel;
static NSScrollView *scroll;
static NSTextView *translation;
static NSButton *copyButton, *replaceButton, *closeButton, *settingsButton;
static NSProgressIndicator *progress;
static NSRect anchor;
static id mouseMonitor, keyMonitor;

static NSTextField *label(NSRect frame, NSFont *font, NSColor *color) {
    NSTextField *view = [NSTextField labelWithString:@""];
    view.frame = frame; view.font = font; view.textColor = color;
    view.lineBreakMode = NSLineBreakByTruncatingTail;
    return view;
}
static NSButton *button(NSString *title, NSRect frame, SEL selector) {
    NSButton *view = [NSButton buttonWithTitle:title target:actions action:selector];
    view.frame = frame; view.bezelStyle = NSBezelStyleRounded;
    view.font = [NSFont systemFontOfSize:12];
    [view setAccessibilityLabel:title];
    return view;
}
static void ensurePanel(void) {
    if (panel) return;
    actions = [RTPopupActions new];
    panel = [[RTPopupPanel alloc] initWithContentRect:NSMakeRect(0, 0, 430, 290)
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    panel.title = @"RustTide 翻译";
    panel.level = NSFloatingWindowLevel;
    panel.hidesOnDeactivate = NO;
    panel.becomesKeyOnlyIfNeeded = YES;
    panel.releasedWhenClosed = NO;
    panel.opaque = NO; panel.backgroundColor = NSColor.clearColor;
    panel.hasShadow = YES;
    panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
    NSVisualEffectView *content = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, 430, 290)];
    content.material = NSVisualEffectMaterialPopover;
    content.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    content.state = NSVisualEffectStateActive;
    content.wantsLayer = YES; content.layer.cornerRadius = 12;
    content.layer.borderWidth = 1;
    content.layer.borderColor = [NSColor.separatorColor colorWithAlphaComponent:0.4].CGColor;
    panel.contentView = content;

    heading = label(NSZeroRect, [NSFont systemFontOfSize:12 weight:NSFontWeightSemibold], NSColor.labelColor);
    sourceLabel = label(NSZeroRect, [NSFont systemFontOfSize:12], NSColor.secondaryLabelColor);
    statusLabel = label(NSZeroRect, [NSFont systemFontOfSize:11], NSColor.secondaryLabelColor);
    statusLabel.maximumNumberOfLines = 2;
    statusLabel.lineBreakMode = NSLineBreakByWordWrapping;
    [statusLabel setAccessibilityLabel:@"翻译状态"];
    scroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    scroll.hasVerticalScroller = YES; scroll.drawsBackground = NO;
    translation = [[NSTextView alloc] initWithFrame:NSMakeRect(0, 0, 398, 170)];
    translation.editable = NO; translation.selectable = YES;
    translation.drawsBackground = NO;
    translation.font = [NSFont systemFontOfSize:15];
    translation.textColor = NSColor.labelColor;
    translation.textContainerInset = NSMakeSize(0, 5);
    translation.textContainer.lineFragmentPadding = 0;
    translation.verticallyResizable = YES;
    translation.horizontallyResizable = NO;
    translation.autoresizingMask = NSViewWidthSizable;
    translation.textContainer.widthTracksTextView = YES;
    [translation setAccessibilityLabel:@"译文"];
    scroll.documentView = translation;
    closeButton = button(@"关闭", NSZeroRect, @selector(close:));
    copyButton = button(@"复制", NSZeroRect, @selector(copy:));
    replaceButton = button(@"替换", NSZeroRect, @selector(replace:));
    settingsButton = button(@"设置", NSZeroRect, @selector(settings:));
    progress = [[NSProgressIndicator alloc] initWithFrame:NSZeroRect];
    progress.style = NSProgressIndicatorStyleSpinning;
    progress.controlSize = NSControlSizeSmall;
    progress.displayedWhenStopped = NO;
    for (NSView *view in @[heading, sourceLabel, statusLabel, scroll, closeButton, copyButton, replaceButton, settingsButton, progress])
        [content addSubview:view];
    // Observe only dismissal gestures. No selection polling or input logging.
    mouseMonitor = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown handler:^(NSEvent *event) {
        (void)event;
        if (panel.isVisible && !NSPointInRect(NSEvent.mouseLocation, panel.frame) && actionHandler)
            actionHandler(1, requestID);
    }];
    keyMonitor = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^(NSEvent *event) {
        if (panel.isVisible && event.keyCode == 53 && actionHandler) actionHandler(1, requestID);
    }];
}
static void place(BOOL compact) {
    CGFloat width = 430, height = compact ? 118 : 290;
    NSScreen *screen = NSScreen.mainScreen;
    for (NSScreen *candidate in NSScreen.screens) {
        if (NSIntersectsRect(candidate.frame, anchor)) { screen = candidate; break; }
    }
    NSRect visible = screen.visibleFrame;
    CGFloat x = MIN(MAX(anchor.origin.x, NSMinX(visible) + 10), NSMaxX(visible) - width - 10);
    CGFloat y = NSMinY(anchor) - height - 10;
    if (y < NSMinY(visible) + 10) y = NSMaxY(anchor) + 10;
    y = MIN(MAX(y, NSMinY(visible) + 10), NSMaxY(visible) - height - 10);
    [panel setFrame:NSMakeRect(x, y, width, height) display:YES];
    heading.frame = NSMakeRect(16, height - 32, 300, 20);
    closeButton.frame = NSMakeRect(350, height - 36, 65, 26);
    sourceLabel.frame = NSMakeRect(16, height - 58, 395, 18);
    scroll.frame = NSMakeRect(16, 70, 398, height - 137);
    scroll.hidden = compact; sourceLabel.hidden = compact;
    progress.frame = NSMakeRect(16, 46, 14, 14);
    statusLabel.frame = NSMakeRect(36, 38, 376, 30);
    settingsButton.frame = NSMakeRect(12, 8, 65, 26);
    copyButton.frame = NSMakeRect(270, 8, 65, 26);
    replaceButton.frame = NSMakeRect(345, 8, 70, 26);
}
void rt_popup_init(void (*handler)(int, uint64_t)) {
    actionHandler = handler;
}
void rt_popup_hide(void) {
    [panel orderOut:nil];
    [progress stopAnimation:nil];
}
void rt_popup_show(const char *payload) {
    @autoreleasepool {
        ensurePanel();
        NSDictionary *value = [NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:payload] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
        requestID = [value[@"id"] unsignedLongLongValue];
        directMode = [value[@"direct"] boolValue];
        anchor = rt_capture_anchor();
        heading.stringValue = [NSString stringWithFormat:@"RustTide · %@ %@", directMode ? @"改写为" : @"译为", value[@"language"] ?: @""];
        NSString *source = value[@"source"] ?: @"";
        sourceLabel.stringValue = [source stringByReplacingOccurrencesOfString:@"\n" withString:@" "];
        translation.string = @"";
        statusLabel.stringValue = @"正在翻译…";
        statusLabel.textColor = NSColor.secondaryLabelColor;
        copyButton.hidden = directMode; replaceButton.hidden = directMode;
        copyButton.enabled = NO; replaceButton.enabled = NO;
        settingsButton.enabled = NO;
        closeButton.title = @"取消";
        [closeButton setAccessibilityLabel:@"取消"];
        place(directMode);
        [progress startAnimation:nil];
        [panel orderFrontRegardless];
    }
}
void rt_popup_update(const char *payload) {
    @autoreleasepool {
        NSDictionary *value = [NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:payload] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
        if ([value[@"id"] unsignedLongLongValue] != requestID || !panel.isVisible) return;
        NSString *kind = value[@"kind"];
        NSString *result = [value[@"result"] isKindOfClass:NSString.class] ? value[@"result"] : nil;
        if (result && (!directMode || [kind isEqualToString:@"error"])) translation.string = result;
        if ([kind isEqualToString:@"stream"]) return;
        [progress stopAnimation:nil];
        settingsButton.enabled = YES; closeButton.title = @"关闭";
        [closeButton setAccessibilityLabel:@"关闭"];
        BOOL failed = [kind isEqualToString:@"error"];
        if (failed && directMode) {
            directMode = NO;
            copyButton.hidden = NO; replaceButton.hidden = NO;
            place(NO);
        }
        statusLabel.stringValue = value[@"message"] ?: @"翻译完成";
        statusLabel.textColor = failed ? NSColor.systemRedColor : NSColor.secondaryLabelColor;
        BOOL complete = [value[@"complete"] boolValue];
        copyButton.enabled = complete && translation.string.length > 0;
        replaceButton.enabled = complete && [value[@"replaceable"] boolValue];
    }
}
