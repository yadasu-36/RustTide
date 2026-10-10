export const languages = ['English', '简体中文', '繁體中文', '日本語', '한국어', 'Español', 'Français', 'Deutsch', 'Português', 'Русский', 'العربية', 'हिन्दी'];
export function initialState() {
  return {source:'', result:'', captureId:null, replaceable:false, app:'', whole:false, busy:false, revision:0};
}
export function editSource(state, source) {
  return {...state, source, captureId:null, replaceable:false, app:'', whole:false, revision:state.revision+1};
}
export function canReplace(state) {
  return !state.busy && state.captureId !== null && state.replaceable && state.result.trim().length > 0;
}
export function acceptResult(state, revision, result) {
  return revision === state.revision ? {...state, result} : state;
}
export function operation(state, event) {
  switch (event.kind) {
    case 'started': return {...state, busy:true, result:'', captureId:null, replaceable:false, revision:state.revision+1};
    case 'captured': return {...state, source:event.source, result:'', captureId:event.captureId, replaceable:event.replaceable, app:event.app, whole:event.whole};
    case 'translated':
      if (event.captureId !== state.captureId) return state;
      return {...state, result:event.result, busy:!!event.direct};
    case 'replaced': return {...state, busy:false, captureId:null, replaceable:false};
    case 'error': return {...state, busy:false};
    default: return state;
  }
}
