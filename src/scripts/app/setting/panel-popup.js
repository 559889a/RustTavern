import { Popup } from '../../popup.js';

export const RUSTTAVERN_PANEL_POPUP_CLASS = 'tt-rusttavern-panel-popup';

const MOBILE_SURFACE_ATTR = 'data-tt-mobile-surface';
const FULLSCREEN_WINDOW_SURFACE = 'fullscreen-window';

export function createRustTavernPanelPopup(content, type, inputValue = '', options = {}) {
    const popup = new Popup(content, type, inputValue, options);
    popup.dlg.classList.add(RUSTTAVERN_PANEL_POPUP_CLASS);
    popup.dlg.setAttribute(MOBILE_SURFACE_ATTR, FULLSCREEN_WINDOW_SURFACE);
    return popup;
}

export function callRustTavernPanelPopup(content, type, inputValue = '', options = {}) {
    return createRustTavernPanelPopup(content, type, inputValue, options).show();
}
