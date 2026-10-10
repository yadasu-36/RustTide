import test from 'node:test';
import assert from 'node:assert/strict';
import {initialState, editSource, canReplace, acceptResult, operation} from './model.js';
test('editing source detaches the original capture target', () => {
  const capture = {...initialState(), source:'原文', result:'translation', captureId:1, replaceable:true};
  assert.equal(canReplace(capture), true);
  const edited = editSource(capture, '修改后的原文');
  assert.equal(canReplace(edited), false);
  assert.equal(edited.captureId, null);
});
test('late response cannot overwrite edited source session', () => {
  const before = initialState();
  const after = editSource(before, 'new text');
  assert.equal(acceptResult(after, before.revision, 'late result'), after);
});
test('direct replacement stays busy until paste finishes', () => {
  let state = operation(initialState(), {kind:'started'});
  state = operation(state, {kind:'captured', source:'你好', captureId:3, replaceable:true, app:'Fixture'});
  state = operation(state, {kind:'translated', result:'Hello', captureId:3, direct:true});
  assert.equal(state.busy, true);
  assert.equal(canReplace(state), false);
  state = operation(state, {kind:'replaced', verified:true});
  assert.equal(state.busy, false);
  assert.equal(state.captureId, null);
});
test('unrelated capture response is ignored and errors allow retry', () => {
  const state = {...initialState(), captureId:9, busy:true};
  assert.equal(operation(state, {kind:'translated', captureId:8, result:'stale'}), state);
  assert.equal(operation(state, {kind:'error'}).busy, false);
});
