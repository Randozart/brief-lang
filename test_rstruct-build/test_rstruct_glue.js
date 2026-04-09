(async function() {
    'use strict';

    const ELEMENT_MAP = {
    };

    console.log('Loading WASM module...');
    const wasm_pkg = await import('./pkg/test_rstruct.js');
    console.log('WASM module loaded, initializing...');
    await wasm_pkg.default();
    console.log('WASM initialized, creating State...');
    const wasm = new wasm_pkg.State();
    console.log('State created, methods available:', Object.keys(wasm).filter(k => k.startsWith('invoke')));

    const TRIGGER_MAP = {
    };

    function attachListeners() {
        console.log('Attaching event listeners...');
        for (const [elId, config] of Object.entries(TRIGGER_MAP)) {
            const el = document.querySelector(ELEMENT_MAP[elId]);
            if (!el) {
                console.warn('Element not found:', elId);
                continue;
            }
            console.log('Attaching', config.event, 'handler to', elId, '->', config.txn);
            el.addEventListener(config.event, () => {
                console.log('Trigger clicked:', config.txn, 'typeof:', typeof wasm[config.txn]);
                try {
                    wasm[config.txn]();
                } catch(e) {
                    console.error('Error calling', config.txn, ':', e);
                }
            });
        }
        console.log('All listeners attached');
    }

    function startPollLoop() {
        function poll() {
            const dispatch = wasm.poll_dispatch();
            console.log('Poll loop, dispatch:', dispatch);
            if (dispatch && dispatch !== '[]') {
                console.log('Applying instructions:', dispatch);
                applyInstructions(JSON.parse(dispatch));
            }
            requestAnimationFrame(poll);
        }
        console.log('Starting poll loop');
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

    attachListeners();
    startPollLoop();
})();
