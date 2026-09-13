/**
 * Shared module between login and main app.
 * Be careful what you import!
 */

const buttonSelectors = [
    '.menu_button',
    '.right_menu_button',
    '.mes_button',
    '.drawer-icon',
    '.inline-drawer-icon',
    '.swipe_left',
    '.swipe_right',
    '.character_select',
    '.tags .tag',
    '.jg-menu .jg-button',
    '.bg_example .mobile-only-menu-toggle',
    '.paginationjs-pages li a',
    '#show_more_messages',
].join(', ');

const listSelectors = [
    '.options-content',
    '.list-group',
    '#rm_print_characters_block',
    '#rm_group_members',
    '#rm_group_add_members',
    '.tag_view_list_tags',
    '.secretKeyManagerList',
    '.recentChatList',
    '.dataMaidCategoryContent',
    '#userList',
    '.bg_list',
].join(', ');

const listItemSelectors = [
    '.options-content .list-group-item',
    '.list-group .list-group-item',
    '#rm_print_characters_block .entity_block',
    '#rm_group_members .group_member',
    '#rm_group_add_members .group_member',
    '.tag_view_list_tags .tag_view_item',
    '.secretKeyManagerList .secretKeyManagerItem',
    '.recentChatList .recentChat',
    '.dataMaidCategoryContent .dataMaidItem',
    '#userList .userSelect',
    '.bg_list .bg_example',
].join(', ');

const toolbarSelectors = [
    '.jg-menu',
].join(', ');

const tabListSelectors = [
    '#bg_tabs .bg_tabs_list',
].join(', ');

const tabItemSelectors = [
    '#bg_tabs .bg_tabs_list .bg_tab_button',
].join(', ');

/** @type {Record<string, (element: Element) => void>} */
const a11yRules = {
    [buttonSelectors]: (element) => {
        element.setAttribute('role', 'button');
    },
    [listSelectors]: (element) => {
        element.setAttribute('role', 'list');
    },
    [listItemSelectors]: (element) => {
        element.setAttribute('role', 'listitem');
    },
    [toolbarSelectors]: (element) => {
        element.setAttribute('role', 'toolbar');
    },
    [tabListSelectors]: (element) => {
        element.setAttribute('role', 'tablist');
    },
    [tabItemSelectors]: (element) => {
        element.setAttribute('role', 'tab');
    },
    '#toast-container .toast': (element) => {
        element.setAttribute('role', 'status');
    },
};

/**
 * Apply accessibility rules to an element.
 * @param {Element} element Element to process.
 */
function applyA11yRules(element) {
    try {
        for (const [selector, rule] of Object.entries(a11yRules)) {
            // Apply if the element directly matches the selector
            if (element.matches(selector)) {
                rule(element);
            }
            // Apply the rule to descendants
            element.querySelectorAll(selector).forEach(rule);
        }
    } catch (error) {
        console.error('Error applying accessibility rules to element:', element, error);
    }
}

/**
 * The roles each rule assigns; used to skip idempotent rewrites of the
 * same role (an attribute write fires MutationObserver records even when
 * the value does not change, which re-enters other observers).
 */
const ruleRoles = Object.values(a11yRules).map(rule => {
    const probe = document.createElement('div');
    rule(probe);
    return probe.getAttribute('role');
});

/**
 * Applies every rule to the element itself and its descendants using one
 * compound query per batch instead of one query per rule per node.
 * @param {Element[]} roots Elements to process (self + descendants).
 */
function applyA11yRulesBatch(roots) {
    try {
        const compound = Object.keys(a11yRules).join(', ');
        const candidates = new Set();
        for (const element of roots) {
            if (element.matches(compound)) {
                candidates.add(element);
            }
            for (const el of element.querySelectorAll(compound)) {
                candidates.add(el);
            }
        }
        if (candidates.size === 0) return;
        // Map candidate -> the first rule whose selector it matches.
        // An element can match several rules' selectors; run every matching
        // rule like applyA11yRules would (last write wins, same as before).
        const rules = Object.entries(a11yRules);
        for (const el of candidates) {
            for (let i = 0; i < rules.length; i++) {
                const [selector, rule] = rules[i];
                if (el.matches(selector) && el.getAttribute('role') !== ruleRoles[i]) {
                    rule(el);
                }
            }
        }
    } catch (error) {
        console.error('Error applying accessibility rules:', error, error);
    }
}

function setAccessibilityObserver() {
    // Apply for existing elements
    applyA11yRules(document.body);

    // Setup observer for dynamic content
    const observer = new MutationObserver((mutationsList) => {
        const roots = [];
        for (const mutation of mutationsList) {
            if (mutation.type === 'childList') {
                for (const addedNode of mutation.addedNodes) {
                    if (addedNode instanceof Element && addedNode.nodeType === Node.ELEMENT_NODE) {
                        roots.push(addedNode);
                    }
                }
            }
        }
        if (roots.length > 0) {
            applyA11yRulesBatch(roots);
        }
    });

    observer.observe(document.body, {
        childList: true,
        subtree: true,
    });
}

export function initAccessibility() {
    setAccessibilityObserver();
}
