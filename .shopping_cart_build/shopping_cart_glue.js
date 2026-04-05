(async function() {
    'use strict';

    const ELEMENT_MAP = {
        'rbv-span-0': '#rbv-span-0',
        'rbv-span-1': '#rbv-span-1',
        'rbv-span-2': '#rbv-span-2',
        'rbv-span-3': '#rbv-span-3',
        'rbv-button-4': '#rbv-button-4',
        'rbv-span-5': '#rbv-span-5',
        'rbv-button-6': '#rbv-button-6',
        'rbv-span-7': '#rbv-span-7',
        'rbv-button-8': '#rbv-button-8',
        'rbv-span-9': '#rbv-span-9',
        'rbv-button-10': '#rbv-button-10',
        'rbv-span-11': '#rbv-span-11',
        'rbv-button-12': '#rbv-button-12',
        'rbv-button-13': '#rbv-button-13',
        'rbv-button-14': '#rbv-button-14',
        'rbv-button-15': '#rbv-button-15',
        'rbv-span-16': '#rbv-span-16',
        'rbv-span-18': '#rbv-span-18',
        'rbv-span-19': '#rbv-span-19',
        'rbv-button-20': '#rbv-button-20',
        'rbv-span-21': '#rbv-span-21',
        'rbv-span-22': '#rbv-span-22',
        'rbv-button-23': '#rbv-button-23',
        'rbv-button-24': '#rbv-button-24',
        'rbv-span-25': '#rbv-span-25',
        'rbv-span-26': '#rbv-span-26',
        'rbv-span-27': '#rbv-span-27',
        'rbv-button-28': '#rbv-button-28',
    };

    const wasm_pkg = await import('./pkg/shopping_cart.js');
    await wasm_pkg.default();
    const wasm = new wasm_pkg.State();

    const TRIGGER_MAP = {
        'rbv-button-4': { event: 'click', txn: 'invoke_add' },
        'rbv-button-6': { event: 'click', txn: 'invoke_add' },
        'rbv-button-8': { event: 'click', txn: 'invoke_add' },
        'rbv-button-10': { event: 'click', txn: 'invoke_add' },
        'rbv-button-12': { event: 'click', txn: 'invoke_select_laptop' },
        'rbv-button-13': { event: 'click', txn: 'invoke_select_keyboard' },
        'rbv-button-14': { event: 'click', txn: 'invoke_select_mouse' },
        'rbv-button-15': { event: 'click', txn: 'invoke_select_monitor' },
        'rbv-button-20': { event: 'click', txn: 'invoke_checkout' },
        'rbv-button-23': { event: 'click', txn: 'invoke_confirm' },
        'rbv-button-24': { event: 'click', txn: 'invoke_reset' },
        'rbv-button-28': { event: 'click', txn: 'invoke_reset' },
    };

    function attachListeners() {
        for (const [elId, config] of Object.entries(TRIGGER_MAP)) {
            const el = document.querySelector(ELEMENT_MAP[elId]);
            if (!el) continue;
            el.addEventListener(config.event, () => {
                wasm[config.txn]();
            });
        }
    }

    function startPollLoop() {
        function poll() {
            const dispatch = wasm.poll_dispatch();
            if (dispatch && dispatch !== '[]') {
                applyInstructions(JSON.parse(dispatch));
            }
            requestAnimationFrame(poll);
        }
        requestAnimationFrame(poll);
    }

    function applyInstructions(instructions) {
        for (const inst of instructions) {
            const el = document.querySelector(ELEMENT_MAP[inst.el]);
            if (!el) continue;
            switch (inst.op) {
                case 'text':
                    el.textContent = inst.value;
                    break;
                case 'show':
                    el.hidden = !inst.visible;
                    break;
                case 'class_add':
                    el.classList.add(inst.class);
                    break;
                case 'class_remove':
                    el.classList.remove(inst.class);
                    break;
            }
        }
    }

    function applyInstructions(instructions) {
        for (const inst of instructions) {
            const el = document.querySelector(ELEMENT_MAP[inst.el]);
            if (!el) continue;
            switch (inst.op) {
                case 'text':
                    el.textContent = inst.value;
                    break;
                case 'show':
                    el.hidden = !inst.visible;
                    break;
                case 'class_add':
                    el.classList.add(inst.class);
                    break;
                case 'class_remove':
                    el.classList.remove(inst.class);
                    break;
            }
        }
    }

    attachListeners();
    startPollLoop();
})();
