import {describe,expect,it,vi} from 'vitest';
import {activateSavedFunctionCard,documentationActionLabel,runSavedFunctionAction} from './App';

describe('saved function interactions',()=>{
  it('opens with click-equivalent keyboard keys only',()=>{
    const open=vi.fn();
    const preventDefault=vi.fn();
    activateSavedFunctionCard({key:'Enter',preventDefault},open);
    activateSavedFunctionCard({key:' ',preventDefault},open);
    activateSavedFunctionCard({key:'Escape',preventDefault},open);
    expect(open).toHaveBeenCalledTimes(2);
    expect(preventDefault).toHaveBeenCalledTimes(2);
  });

  it('isolates document and delete actions from the card click',()=>{
    const stopPropagation=vi.fn();
    const action=vi.fn();
    runSavedFunctionAction({stopPropagation},action);
    expect(stopPropagation).toHaveBeenCalledOnce();
    expect(action).toHaveBeenCalledOnce();
  });

  it('shows generating, retry, and idle labels',()=>{
    expect(documentationActionLabel(true)).toBe('Generating…');
    expect(documentationActionLabel(false,'invalid JSON')).toBe('Retry');
    expect(documentationActionLabel(false)).toBe('Document');
  });
});
