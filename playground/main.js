// Arkade Playground - Main Application
// Import default export for WASM initialization, plus the exported functions
import initWasm, * as wasmApi from './pkg/arkade_compiler.js';
import * as contracts from './contracts.js';

// Projects: collections of related contracts
const projects = {
    stability: {
        name: 'Stability',
        description: 'Synthetic USD stablecoins with on-chain price beacon',
        files: {
            'beacon.ark': contracts.price_beacon,
            'offer.ark': contracts.stability_offer,
            'position.ark': contracts.stable_position,
        }
    }
};

// Single file examples
const examples = {
    runtime_demo: { name: 'RuntimeDemo', code: contracts.runtime_demo },
    single_sig: { name: 'SingleSig', code: contracts.single_sig },
    htlc: { name: 'HTLC', code: contracts.htlc },
    fuji_safe: { name: 'FujiSafe', code: contracts.fuji_safe },
    swap: { name: 'NonInteractiveSwap', code: contracts.non_interactive_swap },
    beacon: { name: 'Beacon', code: contracts.beacon },
};

// Global state
let editor = null;
let wasmReady = false;
let currentProject = null;
let currentFile = null;
let openTabs = [];
let fileContents = {}; // Cache of file contents for each open file
let expandedFolders = new Set(); // Track which folders are expanded
let lastCompiledSource = null; // Source that produced the current output
let lastCompiledArtifactJson = null; // Raw JSON artifact string
let lastRuntimeResult = null; // Parsed runtime result JSON
let runtimeStepIndex = -1;
let runtimeFunctionVariants = [];
let lastRuntimeMatrixResults = [];
let lastCompileDurationMs = 0;
let runtimeTraceView = null;
let runtimeResultStale = false;
let jsonRenderCacheRaw = null;
let jsonRenderCacheHtml = null;
let activeMatrixRunId = 0;
let matrixRunActive = false;

const JSON_SYNTAX_HIGHLIGHT_LIMIT = 250_000;
const ASM_TOKEN_HIGHLIGHT_LIMIT = 14_000;
const MAX_RUNTIME_TRACE_STEPS = 1_600;
const MATRIX_RENDER_BATCH_SIZE = 12;
const MATRIX_PROGRESS_INTERVAL_MS = 60;
const MATRIX_YIELD_INTERVAL_MS = 16;

// ── localStorage persistence ──────────────────────────────────────
const STORAGE_KEY = 'arkade-playground';
const RUNTIME_STORAGE_KEY = 'arkade-playground-runtime-v2';

const RUNTIME_CONTEXT_PRESETS = {
    empty: '',
    simple: JSON.stringify({
        txid: '0101010101010101010101010101010101010101010101010101010101010101',
        version: 2,
        locktime: 0,
        weight: 540,
        currentInputIndex: 0,
        inputs: [
            {
                value: 100000,
                scriptPubKey: '5121031111111111111111111111111111111111111111111111111111111111111111ac',
                sequence: 4294967293,
                outpoint: '0202020202020202020202020202020202020202020202020202020202020202',
                issuance: '',
                assets: []
            }
        ],
        outputs: [
            {
                value: 99500,
                scriptPubKey: '00140000000000000000000000000000000000000000',
                nonce: '0303030303030303030303030303030303030303030303030303030303030303',
                assets: []
            }
        ],
        assetGroups: []
    }, null, 2),
    asset: JSON.stringify({
        txid: '1111111111111111111111111111111111111111111111111111111111111111',
        version: 2,
        locktime: 12,
        weight: 910,
        currentInputIndex: 1,
        inputs: [
            {
                value: 220000,
                scriptPubKey: '512102aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaac',
                sequence: 4294967280,
                outpoint: '2222222222222222222222222222222222222222222222222222222222222222',
                issuance: '0a0b0c0d',
                assets: [
                    {
                        txid: '3333333333333333333333333333333333333333333333333333333333333333',
                        gidx: 0,
                        amount: 5000,
                        data: '746573742d61737365742d696e',
                        control: '636f6e74726f6c2d696e',
                        metadataHash: '4444444444444444444444444444444444444444444444444444444444444444',
                        assetId: '5555555555555555555555555555555555555555555555555555555555555555'
                    }
                ]
            }
        ],
        outputs: [
            {
                value: 218500,
                scriptPubKey: '00148888888888888888888888888888888888888888',
                nonce: '6666666666666666666666666666666666666666666666666666666666666666',
                assets: [
                    {
                        txid: '3333333333333333333333333333333333333333333333333333333333333333',
                        gidx: 0,
                        amount: 4900,
                        data: '746573742d61737365742d6f7574',
                        control: '636f6e74726f6c2d6f7574',
                        metadataHash: '4444444444444444444444444444444444444444444444444444444444444444',
                        assetId: '5555555555555555555555555555555555555555555555555555555555555555'
                    }
                ]
            }
        ],
        assetGroups: [
            {
                txid: '3333333333333333333333333333333333333333333333333333333333333333',
                gidx: 0,
                sumInputs: 5000,
                sumOutputs: 4900,
                numInputs: 1,
                numOutputs: 1,
                control: '636f6e74726f6c2d67726f7570',
                metadataHash: '4444444444444444444444444444444444444444444444444444444444444444',
                assetId: '5555555555555555555555555555555555555555555555555555555555555555'
            }
        ]
    }, null, 2)
};

function saveToStorage() {
    const data = {
        projects: {},
        examples: {}
    };
    // Only save user-created / modified entries
    for (const [id, proj] of Object.entries(projects)) {
        data.projects[id] = { name: proj.name, description: proj.description || '', files: proj.files };
    }
    for (const [id, ex] of Object.entries(examples)) {
        data.examples[id] = { name: ex.name, code: ex.code };
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
}

function loadFromStorage() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) return;
        const data = JSON.parse(raw);
        if (data.projects) {
            for (const [id, proj] of Object.entries(data.projects)) {
                projects[id] = proj;
            }
        }
        if (data.examples) {
            for (const [id, ex] of Object.entries(data.examples)) {
                examples[id] = ex;
            }
        }
    } catch (e) {
        console.warn('Failed to load from localStorage:', e);
    }
}

function loadRuntimeSettings() {
    try {
        const raw = localStorage.getItem(RUNTIME_STORAGE_KEY);
        if (!raw) {
            return null;
        }
        return JSON.parse(raw);
    } catch (err) {
        console.warn('Failed to load runtime settings:', err);
        return null;
    }
}

function saveRuntimeSettings() {
    const bindingsEl = document.getElementById('runtime-bindings');
    const contextEl = document.getElementById('runtime-context');
    const modeEl = document.getElementById('runtime-mode');
    if (!bindingsEl || !contextEl || !modeEl) {
        return;
    }

    const payload = {
        bindings: bindingsEl.value || '',
        context: contextEl.value || '',
        mode: modeEl.value || 'development',
        strictPlaceholders: Boolean(document.getElementById('runtime-strict')?.checked),
        strictTypes: Boolean(document.getElementById('runtime-strict-types')?.checked),
        strictBindings: Boolean(document.getElementById('runtime-strict-bindings')?.checked),
        contextStrict: Boolean(document.getElementById('runtime-context-strict')?.checked)
    };

    localStorage.setItem(RUNTIME_STORAGE_KEY, JSON.stringify(payload));
}

function applyRuntimeSettings(settings) {
    if (!settings) {
        return;
    }

    const bindingsEl = document.getElementById('runtime-bindings');
    const contextEl = document.getElementById('runtime-context');
    const modeEl = document.getElementById('runtime-mode');
    if (bindingsEl && typeof settings.bindings === 'string') {
        bindingsEl.value = settings.bindings;
    }
    if (contextEl && typeof settings.context === 'string') {
        contextEl.value = settings.context;
    }
    if (modeEl && typeof settings.mode === 'string') {
        modeEl.value = settings.mode;
    }

    const strictEl = document.getElementById('runtime-strict');
    const strictTypesEl = document.getElementById('runtime-strict-types');
    const strictBindingsEl = document.getElementById('runtime-strict-bindings');
    const contextStrictEl = document.getElementById('runtime-context-strict');
    if (strictEl) strictEl.checked = Boolean(settings.strictPlaceholders);
    if (strictTypesEl) strictTypesEl.checked = Boolean(settings.strictTypes);
    if (strictBindingsEl) strictBindingsEl.checked = Boolean(settings.strictBindings);
    if (contextStrictEl) contextStrictEl.checked = Boolean(settings.contextStrict);
}

function formatMs(ms) {
    if (!Number.isFinite(ms) || ms < 0) {
        return '0.00 ms';
    }
    return `${ms.toFixed(2)} ms`;
}

function formatNanosAsMs(nanos) {
    const value = Number(nanos);
    if (!Number.isFinite(value) || value < 0) {
        return '0.00 ms';
    }
    return `${(value / 1_000_000).toFixed(2)} ms`;
}

function cancelRuntimeMatrixRun(reason = null) {
    activeMatrixRunId += 1;
    const runBtn = document.getElementById('runtime-run-btn');
    const matrixBtn = document.getElementById('runtime-run-matrix-btn');
    const stopBtn = document.getElementById('runtime-stop-matrix-btn');
    if (runBtn && matrixBtn) {
        setRuntimeBusy(false);
    }
    if (reason) {
        const summaryEl = document.getElementById('runtime-matrix-summary');
        if (summaryEl) {
            summaryEl.classList.remove('running');
            summaryEl.classList.add('complete');
            summaryEl.textContent = reason;
        }
    }
    if (stopBtn) {
        stopBtn.hidden = true;
    }
}

function stopRuntimeMatrix() {
    if (!matrixRunActive) {
        return;
    }
    const renderedRows = document.querySelectorAll('#runtime-matrix-body tr').length;
    cancelRuntimeMatrixRun(`Matrix run stopped by user at ${renderedRows}/${runtimeFunctionVariants.length} paths.`);
}

function waitForNextFrame() {
    return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

// ── URL sharing ───────────────────────────────────────────────────

async function compressCode(text) {
    const stream = new CompressionStream('deflate-raw');
    const writer = stream.writable.getWriter();
    writer.write(new TextEncoder().encode(text));
    writer.close();
    const chunks = [];
    const reader = stream.readable.getReader();
    let result;
    while (!(result = await reader.read()).done) chunks.push(result.value);
    const out = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
    let i = 0;
    for (const c of chunks) { out.set(c, i); i += c.length; }
    let bin = '';
    for (let j = 0; j < out.length; j++) bin += String.fromCharCode(out[j]);
    return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

async function decompressCode(b64url) {
    const bin = atob(b64url.replace(/-/g, '+').replace(/_/g, '/'));
    const data = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) data[i] = bin.charCodeAt(i);
    const stream = new DecompressionStream('deflate-raw');
    const writer = stream.writable.getWriter();
    writer.write(data);
    writer.close();
    const chunks = [];
    const reader = stream.readable.getReader();
    let result;
    while (!(result = await reader.read()).done) chunks.push(result.value);
    const out = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
    let i = 0;
    for (const c of chunks) { out.set(c, i); i += c.length; }
    return new TextDecoder().decode(out);
}

async function shareContract() {
    if (!editor) return;
    const encoded = await compressCode(editor.getValue());
    const url = `${location.origin}${location.pathname}#code=${encoded}`;
    await navigator.clipboard.writeText(url);
    const btn = document.getElementById('share-btn');
    const orig = btn.innerHTML;
    btn.innerHTML = '<i class="fas fa-check"></i>';
    setTimeout(() => { btn.innerHTML = orig; }, 2000);
}

async function loadFromUrl() {
    if (!location.hash.startsWith('#code=')) return null;
    try {
        return await decompressCode(location.hash.slice(6));
    } catch (e) {
        console.warn('Failed to decode shared contract from URL:', e);
        return null;
    }
}

// ── File management helpers ───────────────────────────────────────

function generateId(name) {
    return name.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/(^_|_$)/g, '') || 'untitled';
}

function uniqueId(base, existing) {
    if (!(base in existing)) return base;
    let i = 1;
    while (`${base}_${i}` in existing) i++;
    return `${base}_${i}`;
}

function createFolder(name) {
    const id = uniqueId(generateId(name), projects);
    projects[id] = { name, description: '', files: {} };
    expandedFolders.add(id);
    saveToStorage();
    renderFileTree();
    return id;
}

function createFileInFolder(folderId, fileName) {
    if (!fileName.endsWith('.ark')) fileName += '.ark';
    const project = projects[folderId];
    if (!project) return;
    if (project.files[fileName]) return; // already exists
    const defaultCode = `// ${fileName}\n\noptions {\n  server = serverPk;\n  exit = 144;\n}\n\ncontract MyContract(\n  pubkey user\n) {\n  function spend(signature userSig) {\n    require(checkSig(userSig, user));\n  }\n}\n`;
    project.files[fileName] = defaultCode;
    saveToStorage();
    selectProjectFile(folderId, fileName);
}

function createStandaloneFile(name) {
    if (!name.endsWith('.ark')) name += '.ark';
    const displayName = name.replace(/\.ark$/, '');
    const id = uniqueId(generateId(displayName), examples);
    const defaultCode = `// ${displayName} Contract\n\noptions {\n  server = serverPk;\n  exit = 144;\n}\n\ncontract ${displayName}(\n  pubkey user\n) {\n  function spend(signature userSig) {\n    require(checkSig(userSig, user));\n  }\n}\n`;
    examples[id] = { name: displayName, code: defaultCode };
    expandedFolders.add('_examples');
    saveToStorage();
    selectExample(id);
}

function renameFolder(folderId, newName) {
    const project = projects[folderId];
    if (!project) return;
    project.name = newName;
    saveToStorage();
    renderFileTree();
}

function renameFileInFolder(folderId, oldName, newName) {
    if (!newName.endsWith('.ark')) newName += '.ark';
    const project = projects[folderId];
    if (!project || !project.files[oldName] || oldName === newName) return;
    if (project.files[newName]) return; // target exists

    project.files[newName] = project.files[oldName];
    delete project.files[oldName];

    // Update tabs and cache
    const oldTabId = `${folderId}:${oldName}`;
    const newTabId = `${folderId}:${newName}`;
    const tab = openTabs.find(t => t.id === oldTabId);
    if (tab) {
        tab.id = newTabId;
        tab.file = newName;
        tab.name = newName;
        if (fileContents[oldTabId] !== undefined) {
            fileContents[newTabId] = fileContents[oldTabId];
            delete fileContents[oldTabId];
        }
    }
    if (currentProject === folderId && currentFile === oldName) {
        currentFile = newName;
    }

    saveToStorage();
    updateFileTabs();
    renderFileTree();
    updateCurrentFileName(newName);
}

function renameExample(exampleId, newName) {
    const example = examples[exampleId];
    if (!example) return;
    example.name = newName;

    // Update tab display name
    const tab = openTabs.find(t => t.id === exampleId);
    if (tab) {
        tab.name = `${newName}.ark`;
    }
    if (currentProject === null && currentFile === exampleId) {
        updateCurrentFileName(`${newName}.ark`);
    }

    saveToStorage();
    updateFileTabs();
    renderFileTree();
}

function deleteFolder(folderId) {
    if (!projects[folderId]) return;
    // Close all tabs from this folder
    const tabsToClose = openTabs.filter(t => t.project === folderId).map(t => t.id);
    for (const tabId of tabsToClose) {
        closeTab(tabId);
    }
    delete projects[folderId];
    expandedFolders.delete(folderId);
    saveToStorage();
    renderFileTree();
}

function deleteFileInFolder(folderId, fileName) {
    const project = projects[folderId];
    if (!project || !project.files[fileName]) return;
    const tabId = `${folderId}:${fileName}`;
    closeTab(tabId);
    delete project.files[fileName];
    saveToStorage();
    renderFileTree();
}

function deleteExample(exampleId) {
    if (!examples[exampleId]) return;
    closeTab(exampleId);
    delete examples[exampleId];
    saveToStorage();
    renderFileTree();
}

function moveProjectFileToFolder(fromFolderId, fileName, toFolderId) {
    const fromProject = projects[fromFolderId];
    const toProject   = projects[toFolderId];
    if (!fromProject || !toProject) return;
    if (fromFolderId === toFolderId) return;
    if (!fromProject.files[fileName]) return;

    // Resolve name collision in destination
    const baseName  = fileName.replace(/\.ark$/, '');
    const destFiles = toProject.files;
    let   destName  = fileName;
    if (destName in destFiles) {
        let i = 2;
        while (`${baseName}_${i}.ark` in destFiles) i++;
        destName = `${baseName}_${i}.ark`;
    }

    // Read latest content (prefer in-memory cache to preserve unsaved edits)
    const oldTabId = `${fromFolderId}:${fileName}`;
    const content  = fileContents[oldTabId] !== undefined
                       ? fileContents[oldTabId]
                       : fromProject.files[fileName];

    // Mutate source data
    toProject.files[destName] = content;
    delete fromProject.files[fileName];

    // Remap open tab if present
    const newTabId = `${toFolderId}:${destName}`;
    const tab = openTabs.find(t => t.id === oldTabId);
    if (tab) {
        tab.id      = newTabId;
        tab.project = toFolderId;
        tab.file    = destName;
        tab.name    = destName;
        fileContents[newTabId] = content;
        delete fileContents[oldTabId];
    }

    // Patch active editor state
    if (currentProject === fromFolderId && currentFile === fileName) {
        currentProject = toFolderId;
        currentFile    = destName;
        updateCurrentFileName(destName);
    }

    expandedFolders.add(toFolderId);
    saveToStorage();
    updateFileTabs();
    renderFileTree();
}

function moveProjectFileToExamples(folderId, fileName) {
    const project = projects[folderId];
    if (!project || !project.files[fileName]) return;

    // Read latest content
    const oldTabId = `${folderId}:${fileName}`;
    const content  = fileContents[oldTabId] !== undefined
                       ? fileContents[oldTabId]
                       : project.files[fileName];

    // Derive example id
    const displayName = fileName.replace(/\.ark$/, '');
    const baseId      = generateId(displayName);
    const exampleId   = uniqueId(baseId, examples);

    // Mutate source data
    examples[exampleId] = { name: displayName, code: content };
    delete project.files[fileName];

    // Remap open tab if present
    const tab = openTabs.find(t => t.id === oldTabId);
    if (tab) {
        tab.id      = exampleId;
        tab.project = null;
        tab.file    = exampleId;
        tab.name    = `${displayName}.ark`;
        fileContents[exampleId] = content;
        delete fileContents[oldTabId];
    }

    // Patch active editor state
    if (currentProject === folderId && currentFile === fileName) {
        currentProject = null;
        currentFile    = exampleId;
        updateCurrentFileName(`${displayName}.ark`);
    }

    expandedFolders.add('_examples');
    saveToStorage();
    updateFileTabs();
    renderFileTree();
}

function moveExampleToFolder(exampleId, toFolderId) {
    const example   = examples[exampleId];
    const toProject = projects[toFolderId];
    if (!example || !toProject) return;

    // Read latest content
    const oldTabId = exampleId;
    const content  = fileContents[oldTabId] !== undefined
                       ? fileContents[oldTabId]
                       : example.code;

    // Resolve destination file name
    const baseName = example.name;
    let   destName = `${baseName}.ark`;
    if (destName in toProject.files) {
        let i = 2;
        while (`${baseName}_${i}.ark` in toProject.files) i++;
        destName = `${baseName}_${i}.ark`;
    }

    // Mutate source data
    toProject.files[destName] = content;
    delete examples[exampleId];

    // Remap open tab if present
    const newTabId = `${toFolderId}:${destName}`;
    const tab = openTabs.find(t => t.id === oldTabId);
    if (tab) {
        tab.id      = newTabId;
        tab.project = toFolderId;
        tab.file    = destName;
        tab.name    = destName;
        fileContents[newTabId] = content;
        delete fileContents[oldTabId];
    }

    // Patch active editor state
    if (currentProject === null && currentFile === exampleId) {
        currentProject = toFolderId;
        currentFile    = destName;
        updateCurrentFileName(destName);
    }

    expandedFolders.add(toFolderId);
    saveToStorage();
    updateFileTabs();
    renderFileTree();
}

// ── Context menu ──────────────────────────────────────────────────

let contextMenuTarget = null;

function showContextMenu(e, items) {
    e.preventDefault();
    const menu = document.getElementById('context-menu');
    let html = '';
    for (const item of items) {
        if (item.separator) {
            html += '<div class="context-menu-separator"></div>';
        } else {
            const cls = item.danger ? 'context-menu-item danger' : 'context-menu-item';
            html += `<div class="${cls}" data-action="${item.action}">
                <i class="fas ${item.icon}"></i> ${item.label}
            </div>`;
        }
    }
    menu.innerHTML = html;

    // Position
    menu.style.left = `${e.clientX}px`;
    menu.style.top = `${e.clientY}px`;
    menu.classList.add('visible');

    // Ensure menu stays within viewport
    requestAnimationFrame(() => {
        const rect = menu.getBoundingClientRect();
        if (rect.right > window.innerWidth) {
            menu.style.left = `${window.innerWidth - rect.width - 4}px`;
        }
        if (rect.bottom > window.innerHeight) {
            menu.style.top = `${window.innerHeight - rect.height - 4}px`;
        }
    });

    // Action handlers
    menu.querySelectorAll('.context-menu-item').forEach(el => {
        el.addEventListener('click', () => {
            hideContextMenu();
            handleContextAction(el.dataset.action);
        });
    });
}

function hideContextMenu() {
    document.getElementById('context-menu').classList.remove('visible');
}

function handleContextAction(action) {
    const t = contextMenuTarget;
    contextMenuTarget = null;
    if (!t) return;

    switch (action) {
        case 'new-file-in-folder':
            promptNewFileInFolder(t.folderId);
            break;
        case 'rename-folder':
            startInlineRename('folder', t.folderId);
            break;
        case 'delete-folder':
            if (confirm(`Delete folder "${projects[t.folderId]?.name}" and all its files?`)) {
                deleteFolder(t.folderId);
            }
            break;
        case 'rename-file':
            if (t.folderId) {
                startInlineRename('project-file', t.folderId, t.fileName);
            } else {
                startInlineRename('example', t.exampleId);
            }
            break;
        case 'delete-file':
            if (t.folderId) {
                if (confirm(`Delete "${t.fileName}"?`)) {
                    deleteFileInFolder(t.folderId, t.fileName);
                }
            } else {
                if (confirm(`Delete "${examples[t.exampleId]?.name}.ark"?`)) {
                    deleteExample(t.exampleId);
                }
            }
            break;
        case 'new-file':
            promptNewStandaloneFile();
            break;
        case 'new-folder':
            promptNewFolder();
            break;
    }
}

// ── Inline rename ─────────────────────────────────────────────────

function startInlineRename(type, id, fileName) {
    renderFileTree(); // reset any existing rename inputs
    let el;

    if (type === 'folder') {
        el = document.querySelector(`.tree-folder[data-folder="${id}"]`);
        if (!el) return;
        const currentName = projects[id]?.name || id;
        const input = document.createElement('input');
        input.className = 'tree-rename-input';
        input.value = currentName;
        el.textContent = '';
        el.appendChild(input);
        input.focus();
        input.select();
        const commit = () => {
            const val = input.value.trim();
            if (val && val !== currentName) renameFolder(id, val);
            else renderFileTree();
        };
        input.addEventListener('blur', commit);
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') { e.preventDefault(); input.blur(); }
            if (e.key === 'Escape') { input.value = currentName; input.blur(); }
        });
    } else if (type === 'project-file') {
        el = document.querySelector(`.tree-item[data-project="${id}"][data-file="${fileName}"]`);
        if (!el) return;
        const input = document.createElement('input');
        input.className = 'tree-rename-input';
        input.value = fileName;
        el.textContent = '';
        el.appendChild(input);
        input.focus();
        input.select();
        const commit = () => {
            let val = input.value.trim();
            if (val && val !== fileName) renameFileInFolder(id, fileName, val);
            else renderFileTree();
        };
        input.addEventListener('blur', commit);
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') { e.preventDefault(); input.blur(); }
            if (e.key === 'Escape') { input.value = fileName; input.blur(); }
        });
    } else if (type === 'example') {
        el = document.querySelector(`.tree-item[data-example="${id}"]`);
        if (!el) return;
        const currentName = examples[id]?.name || id;
        const input = document.createElement('input');
        input.className = 'tree-rename-input';
        input.value = currentName;
        el.textContent = '';
        el.appendChild(input);
        input.focus();
        input.select();
        const commit = () => {
            const val = input.value.trim();
            if (val && val !== currentName) renameExample(id, val);
            else renderFileTree();
        };
        input.addEventListener('blur', commit);
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') { e.preventDefault(); input.blur(); }
            if (e.key === 'Escape') { input.value = currentName; input.blur(); }
        });
    }
}

// ── Prompt dialogs for new items ──────────────────────────────────

function promptNewFolder() {
    const name = prompt('New folder name:');
    if (name && name.trim()) createFolder(name.trim());
}

function promptNewFileInFolder(folderId) {
    const name = prompt('New file name (e.g. contract.ark):');
    if (name && name.trim()) createFileInFolder(folderId, name.trim());
}

function promptNewStandaloneFile() {
    const name = prompt('New file name (e.g. MyContract.ark):');
    if (name && name.trim()) createStandaloneFile(name.trim());
}

// Initialize WASM module
async function initCompiler() {
    try {
        await initWasm();
        if (typeof wasmApi.init === 'function') {
            wasmApi.init();
        }
        wasmReady = true;

        const ver = typeof wasmApi.version === 'function' ? wasmApi.version() : 'unknown';
        document.getElementById('compiler-version').textContent = `v${ver}`;
        document.getElementById('footer-version').textContent = `v${ver}`;

        // Show button as ready to compile
        markDirty();
    } catch (err) {
        console.error('Failed to initialize WASM:', err);
        showError('Failed to load compiler. Make sure the WASM module is built.');
    }
}

// Render file tree
function renderFileTree() {
    const container = document.getElementById('file-tree');
    let html = '';

    // Examples folder (contains project sub-folders + standalone examples)
    const examplesExpanded = expandedFolders.has('_examples');
    html += `<div class="tree-folder" data-folder="_examples">
        <i class="fas ${examplesExpanded ? 'fa-chevron-down' : 'fa-chevron-right'}"></i>
        <i class="fas fa-folder${examplesExpanded ? '-open' : ''}"></i>
        Examples
    </div>`;
    html += `<div class="tree-folder-content ${examplesExpanded ? 'expanded' : ''}" data-folder="_examples">`;

    // Project sub-folders nested inside Examples
    for (const [id, project] of Object.entries(projects)) {
        const isExpanded = expandedFolders.has(id);
        html += `<div class="tree-folder" data-folder="${id}">
            <i class="fas ${isExpanded ? 'fa-chevron-down' : 'fa-chevron-right'}"></i>
            <i class="fas fa-folder${isExpanded ? '-open' : ''}"></i>
            ${project.name}
        </div>`;
        html += `<div class="tree-folder-content ${isExpanded ? 'expanded' : ''}" data-folder="${id}">`;
        for (const fileName of Object.keys(project.files)) {
            const isActive = currentProject === id && currentFile === fileName;
            html += `<div class="tree-item ${isActive ? 'active' : ''}" data-project="${id}" data-file="${fileName}" draggable="true">
                <i class="fas fa-file-code"></i>
                ${fileName}
            </div>`;
        }
        html += '</div>';
    }

    // Standalone examples
    for (const [id, example] of Object.entries(examples)) {
        const isActive = currentProject === null && currentFile === id;
        html += `<div class="tree-item ${isActive ? 'active' : ''}" data-example="${id}" draggable="true">
            <i class="fas fa-file-code"></i>
            ${example.name}.ark
        </div>`;
    }
    html += '</div>';

    container.innerHTML = html;

    // Add click handlers for folders
    container.querySelectorAll('.tree-folder').forEach(folder => {
        folder.addEventListener('click', () => {
            const folderId = folder.dataset.folder;
            toggleFolder(folderId);
        });
        // Right-click context menu for folders
        folder.addEventListener('contextmenu', (e) => {
            e.stopPropagation();
            const folderId = folder.dataset.folder;
            if (folderId === '_examples') {
                contextMenuTarget = { type: 'examples-folder' };
                showContextMenu(e, [
                    { action: 'new-file', icon: 'fa-file-circle-plus', label: 'New File' }
                ]);
            } else {
                contextMenuTarget = { type: 'folder', folderId };
                showContextMenu(e, [
                    { action: 'new-file-in-folder', icon: 'fa-file-circle-plus', label: 'New File' },
                    { separator: true },
                    { action: 'rename-folder', icon: 'fa-pen', label: 'Rename' },
                    { action: 'delete-folder', icon: 'fa-trash', label: 'Delete', danger: true }
                ]);
            }
        });
        // Drag-and-drop: folder header as drop target
        folder.addEventListener('dragover', (e) => {
            if (!e.dataTransfer.types.includes('text/plain')) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
            folder.classList.add('drag-over');
        });
        folder.addEventListener('dragleave', (e) => {
            if (folder.contains(e.relatedTarget)) return;
            folder.classList.remove('drag-over');
        });
        folder.addEventListener('drop', (e) => {
            e.preventDefault();
            e.stopPropagation();
            folder.classList.remove('drag-over');
            const raw = e.dataTransfer.getData('text/plain');
            if (!raw) return;
            const targetFolderId = folder.dataset.folder;
            const parts = raw.split('|');
            if (parts[0] === 'project-file') {
                const fromFolderId = parts[1];
                const fileName     = parts[2];
                if (targetFolderId === '_examples') {
                    moveProjectFileToExamples(fromFolderId, fileName);
                } else {
                    moveProjectFileToFolder(fromFolderId, fileName, targetFolderId);
                }
            } else if (parts[0] === 'example') {
                if (targetFolderId === '_examples') return;
                moveExampleToFolder(parts[1], targetFolderId);
            }
        });
    });

    // Drag-and-drop: folder content area as drop target
    container.querySelectorAll('.tree-folder-content').forEach(content => {
        const folderId = content.dataset.folder;
        content.addEventListener('dragover', (e) => {
            if (!e.dataTransfer.types.includes('text/plain')) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
            content.classList.add('drag-over');
        });
        content.addEventListener('dragleave', (e) => {
            if (content.contains(e.relatedTarget)) return;
            content.classList.remove('drag-over');
        });
        content.addEventListener('drop', (e) => {
            e.preventDefault();
            e.stopPropagation();
            content.classList.remove('drag-over');
            const raw = e.dataTransfer.getData('text/plain');
            if (!raw) return;
            const parts = raw.split('|');
            if (parts[0] === 'project-file') {
                const fromFolderId = parts[1];
                const fileName     = parts[2];
                if (folderId === '_examples') {
                    moveProjectFileToExamples(fromFolderId, fileName);
                } else {
                    moveProjectFileToFolder(fromFolderId, fileName, folderId);
                }
            } else if (parts[0] === 'example') {
                if (folderId === '_examples') return;
                moveExampleToFolder(parts[1], folderId);
            }
        });
    });

    container.querySelectorAll('.tree-item[data-project]').forEach(item => {
        item.addEventListener('click', (e) => {
            e.stopPropagation();
            selectProjectFile(item.dataset.project, item.dataset.file);
        });
        // Right-click context menu for project files
        item.addEventListener('contextmenu', (e) => {
            e.stopPropagation();
            contextMenuTarget = { type: 'project-file', folderId: item.dataset.project, fileName: item.dataset.file };
            showContextMenu(e, [
                { action: 'rename-file', icon: 'fa-pen', label: 'Rename' },
                { action: 'delete-file', icon: 'fa-trash', label: 'Delete', danger: true }
            ]);
        });
        // Drag-and-drop: project file as drag source
        item.addEventListener('dragstart', (e) => {
            e.stopPropagation();
            e.dataTransfer.setData('text/plain', `project-file|${item.dataset.project}|${item.dataset.file}`);
            e.dataTransfer.effectAllowed = 'move';
            item.classList.add('dragging');
        });
        item.addEventListener('dragend', () => {
            item.classList.remove('dragging');
            container.querySelectorAll('.drag-over').forEach(el => el.classList.remove('drag-over'));
        });
    });

    container.querySelectorAll('.tree-item[data-example]').forEach(item => {
        item.addEventListener('click', (e) => {
            e.stopPropagation();
            selectExample(item.dataset.example);
        });
        // Right-click context menu for example files
        item.addEventListener('contextmenu', (e) => {
            e.stopPropagation();
            contextMenuTarget = { type: 'example', exampleId: item.dataset.example };
            showContextMenu(e, [
                { action: 'rename-file', icon: 'fa-pen', label: 'Rename' },
                { action: 'delete-file', icon: 'fa-trash', label: 'Delete', danger: true }
            ]);
        });
        // Drag-and-drop: example as drag source
        item.addEventListener('dragstart', (e) => {
            e.stopPropagation();
            e.dataTransfer.setData('text/plain', `example|${item.dataset.example}`);
            e.dataTransfer.effectAllowed = 'move';
            item.classList.add('dragging');
        });
        item.addEventListener('dragend', () => {
            item.classList.remove('dragging');
            container.querySelectorAll('.drag-over').forEach(el => el.classList.remove('drag-over'));
        });
    });
}

// Toggle folder expansion
function toggleFolder(folderId) {
    if (expandedFolders.has(folderId)) {
        expandedFolders.delete(folderId);
    } else {
        expandedFolders.add(folderId);
    }
    renderFileTree();
}

// Select a file from a project
function selectProjectFile(projectId, fileName) {
    // Save current file content
    saveCurrentFile();

    // Expand the folder
    expandedFolders.add(projectId);

    currentProject = projectId;
    currentFile = fileName;

    const project = projects[projectId];
    const code = project.files[fileName];

    // Update open tabs
    const tabId = `${projectId}:${fileName}`;
    if (!openTabs.find(t => t.id === tabId)) {
        openTabs.push({ id: tabId, project: projectId, file: fileName, name: fileName });
    }
    fileContents[tabId] = code;

    if (editor) {
        editor.setValue(code);
    }

    updateFileTabs();
    renderFileTree();
    updateCurrentFileName(fileName);
    lastCompiledSource = null;
    markDirty();
}

// Select a single-file example
function selectExample(exampleId) {
    // Save current file content
    saveCurrentFile();

    // Expand the examples folder
    expandedFolders.add('_examples');

    currentProject = null;
    currentFile = exampleId;

    const example = examples[exampleId];

    // Update open tabs
    const tabId = exampleId;
    if (!openTabs.find(t => t.id === tabId)) {
        openTabs.push({ id: tabId, project: null, file: exampleId, name: `${example.name}.ark` });
    }
    fileContents[tabId] = example.code;

    if (editor) {
        editor.setValue(example.code);
    }

    updateFileTabs();
    renderFileTree();
    updateCurrentFileName(`${example.name}.ark`);
    lastCompiledSource = null;
    markDirty();
}

// Save current file content to cache and source data
function saveCurrentFile() {
    if (!editor) return;

    let tabId;
    if (currentProject) {
        tabId = `${currentProject}:${currentFile}`;
    } else if (currentFile) {
        tabId = currentFile;
    }

    if (tabId) {
        const content = editor.getValue();
        fileContents[tabId] = content;

        // Persist back to source data
        if (currentProject && projects[currentProject]) {
            projects[currentProject].files[currentFile] = content;
        } else if (currentFile && examples[currentFile]) {
            examples[currentFile].code = content;
        }
        saveToStorage();
    }
}

// Update file tabs UI
function updateFileTabs() {
    const container = document.getElementById('file-tabs');
    if (openTabs.length === 0) {
        container.innerHTML = '';
        return;
    }

    let activeTabId;
    if (currentProject) {
        activeTabId = `${currentProject}:${currentFile}`;
    } else {
        activeTabId = currentFile;
    }

    let html = '';
    for (const tab of openTabs) {
        const isActive = tab.id === activeTabId;
        html += `<span class="file-tab ${isActive ? 'active' : ''}" data-tab="${tab.id}">
            <i class="fas fa-file-code"></i>
            <span class="tab-name">${tab.name}</span>
            <i class="fas fa-times tab-close" data-tab="${tab.id}"></i>
        </span>`;
    }
    container.innerHTML = html;

    // Add click handlers for tabs
    container.querySelectorAll('.file-tab').forEach(tabEl => {
        tabEl.addEventListener('click', (e) => {
            if (e.target.classList.contains('tab-close')) {
                closeTab(tabEl.dataset.tab);
            } else {
                switchToTab(tabEl.dataset.tab);
            }
        });
    });
}

// Switch to a tab
function switchToTab(tabId) {
    saveCurrentFile();

    const tab = openTabs.find(t => t.id === tabId);
    if (!tab) return;

    currentProject = tab.project;
    currentFile = tab.file;

    const content = fileContents[tabId];
    if (content !== undefined && editor) {
        editor.setValue(content);
    }

    updateFileTabs();
    renderFileTree();
    updateCurrentFileName(tab.name);
    lastCompiledSource = null;
    markDirty();
}

// Close a tab
function closeTab(tabId) {
    const idx = openTabs.findIndex(t => t.id === tabId);
    if (idx === -1) return;

    openTabs.splice(idx, 1);
    delete fileContents[tabId];

    // If closing active tab, switch to another
    const activeTabId = currentProject ? `${currentProject}:${currentFile}` : currentFile;
    if (tabId === activeTabId) {
        if (openTabs.length > 0) {
            const newTab = openTabs[Math.min(idx, openTabs.length - 1)];
            switchToTab(newTab.id);
            return;
        } else {
            // No tabs left, load default
            selectExample('single_sig');
            return;
        }
    }

    updateFileTabs();
}

// Update current file name display
function updateCurrentFileName(name) {
    document.getElementById('current-file').textContent = name;
}

// Initialize Monaco Editor
function initMonaco() {
    require.config({
        paths: {
            'vs': 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs'
        }
    });

    require(['vs/editor/editor.main'], function() {
        // Register Arkade language
        monaco.languages.register({ id: 'arkade' });

        // Set tokenizer (Monarch definition)
        monaco.languages.setMonarchTokensProvider('arkade', window.arkadeMonarch);

        // Set language configuration
        monaco.languages.setLanguageConfiguration('arkade', window.arkadeLanguageConfig);

        // Register completions
        monaco.languages.registerCompletionItemProvider('arkade', {
            provideCompletionItems: (model, position) => {
                const suggestions = window.arkadeCompletions.map(item => ({
                    label: item.label,
                    kind: monaco.languages.CompletionItemKind[item.kind] || monaco.languages.CompletionItemKind.Text,
                    insertText: item.insertText,
                    insertTextRules: item.insertTextRules ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet : undefined,
                    detail: item.detail || '',
                    range: {
                        startLineNumber: position.lineNumber,
                        startColumn: position.column,
                        endLineNumber: position.lineNumber,
                        endColumn: position.column
                    }
                }));
                return { suggestions };
            }
        });

        // Define theme
        monaco.editor.defineTheme('arkade-dark', window.arkadeTheme);

        // Create editor
        editor = monaco.editor.create(document.getElementById('editor'), {
            value: examples.runtime_demo.code,
            language: 'arkade',
            theme: 'arkade-dark',
            automaticLayout: true,
            minimap: { enabled: false },
            fontSize: 14,
            lineNumbers: 'on',
            renderLineHighlight: 'all',
            scrollBeyondLastLine: false,
            wordWrap: 'on',
            tabSize: 2,
            insertSpaces: true,
            folding: true,
            bracketPairColorization: { enabled: true }
        });

        // Keyboard shortcut: Ctrl+Enter to compile
        editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
            doCompile();
        });

        // Mark dirty on change — no auto-compile
        editor.onDidChangeModelContent(() => {
            if (editor.getValue() !== lastCompiledSource) {
                markDirty();
            }
        });

        // Load shared contract from URL hash if present
        window._urlCodePromise.then(urlCode => {
            if (urlCode) {
                const id = uniqueId('shared', examples);
                examples[id] = { name: 'Shared', code: urlCode };
                saveToStorage();
                selectExample(id);
                history.replaceState(null, '', location.pathname + location.search);
            }
        });

        // Initialize WASM after editor is ready
        initCompiler();
    });
}

function buildRuntimeTraceView(runtimeJson) {
    const telemetry = Array.isArray(runtimeJson?.telemetry) ? runtimeJson.telemetry : [];
    if (telemetry.length === 0) {
        return {
            entries: [],
            beforeCache: [],
            afterCache: [],
            totalSteps: 0,
            stride: 1,
        };
    }

    const stride = telemetry.length > MAX_RUNTIME_TRACE_STEPS
        ? Math.ceil(telemetry.length / MAX_RUNTIME_TRACE_STEPS)
        : 1;
    const entries = [];
    for (let i = 0; i < telemetry.length; i += stride) {
        entries.push({ sourceIndex: i, step: telemetry[i] });
    }
    if (entries[entries.length - 1]?.sourceIndex !== telemetry.length - 1) {
        entries.push({ sourceIndex: telemetry.length - 1, step: telemetry[telemetry.length - 1] });
    }

    return {
        entries,
        beforeCache: new Array(entries.length),
        afterCache: new Array(entries.length),
        totalSteps: telemetry.length,
        stride,
    };
}

function markRuntimeFresh() {
    runtimeResultStale = false;
}

function markRuntimeStale(reason) {
    if (!lastRuntimeResult && !lastRuntimeMatrixResults.length) {
        return;
    }
    if (runtimeResultStale) {
        return;
    }
    runtimeResultStale = true;
    const summaryEl = document.getElementById('runtime-summary');
    summaryEl.classList.remove('success', 'error');
    summaryEl.classList.add('stale');
    summaryEl.textContent = `Stale result: ${reason}\nPress Execute to refresh runtime diagnostics.`;
}

function setRuntimeMetricDefaults() {
    document.getElementById('runtime-metric-total-ms').textContent = '0.00 ms';
    document.getElementById('runtime-metric-vm-ms').textContent = '0.00 ms';
    document.getElementById('runtime-metric-steps').textContent = '0';
    document.getElementById('runtime-metric-policy').textContent = '0';
    document.getElementById('runtime-trace').textContent = 'No trace yet.';
    document.getElementById('runtime-policy').textContent = 'No policy snapshot yet.';
}

function runtimeVariantLabel(variant) {
    return `${variant.name} (${variant.serverVariant ? 'cooperative' : 'exit'})`;
}

function refreshRuntimeActionButtons(isBusy) {
    const hasVariants = runtimeFunctionVariants.length > 0;
    const runBtn = document.getElementById('runtime-run-btn');
    const matrixBtn = document.getElementById('runtime-run-matrix-btn');
    const fillBtn = document.getElementById('runtime-fill-bindings-btn');
    const stopBtn = document.getElementById('runtime-stop-matrix-btn');
    runBtn.disabled = isBusy || !hasVariants;
    matrixBtn.disabled = isBusy || !hasVariants;
    fillBtn.disabled = isBusy || !hasVariants;
    if (stopBtn) {
        stopBtn.disabled = !matrixRunActive;
    }
}

function setRuntimeBusy(isBusy, mode = null) {
    const runBtn = document.getElementById('runtime-run-btn');
    const matrixBtn = document.getElementById('runtime-run-matrix-btn');
    const stopBtn = document.getElementById('runtime-stop-matrix-btn');
    if (!runBtn.dataset.defaultLabel) runBtn.dataset.defaultLabel = runBtn.innerHTML;
    if (!matrixBtn.dataset.defaultLabel) matrixBtn.dataset.defaultLabel = matrixBtn.innerHTML;
    if (isBusy && mode === 'matrix') {
        matrixRunActive = true;
    } else if (!isBusy) {
        matrixRunActive = false;
    }

    if (isBusy) {
        if (mode === 'single') {
            runBtn.innerHTML = '<i class="fas fa-circle-notch fa-spin"></i> Executing';
        }
        if (mode === 'matrix') {
            matrixBtn.innerHTML = '<i class="fas fa-circle-notch fa-spin"></i> Running';
        }
    } else {
        runBtn.innerHTML = runBtn.dataset.defaultLabel;
        matrixBtn.innerHTML = matrixBtn.dataset.defaultLabel;
    }
    if (stopBtn) {
        stopBtn.hidden = !matrixRunActive;
    }

    refreshRuntimeActionButtons(isBusy);
}

function resetRuntimeUI(message) {
    cancelRuntimeMatrixRun();
    lastRuntimeResult = null;
    runtimeStepIndex = -1;
    runtimeTraceView = null;
    markRuntimeFresh();

    const summaryEl = document.getElementById('runtime-summary');
    summaryEl.classList.remove('success', 'error', 'stale');
    summaryEl.textContent = message;

    document.getElementById('runtime-step-label').textContent = 'Step 0 / 0';
    document.getElementById('runtime-step').textContent = '';
    document.getElementById('runtime-main-stack').textContent = '';
    document.getElementById('runtime-alt-stack').textContent = '';
    document.getElementById('runtime-prev-step').disabled = true;
    document.getElementById('runtime-next-step').disabled = true;

    const slider = document.getElementById('runtime-step-slider');
    slider.value = '0';
    slider.max = '0';
    slider.disabled = true;

    lastRuntimeMatrixResults = [];
    const matrixSummaryEl = document.getElementById('runtime-matrix-summary');
    matrixSummaryEl.classList.remove('running', 'complete');
    matrixSummaryEl.textContent = 'Run matrix to benchmark every function variant.';
    document.getElementById('runtime-matrix-table').textContent = '';

    setRuntimeMetricDefaults();
}

function invalidateRuntimeResult(reason) {
    markRuntimeStale(reason);
}

function runtimeBindingDefaultForInput(rawType, inputName) {
    const type = String(rawType || '').toLowerCase();
    if (type.includes('signature')) return 'hex:30440220...';
    if (type.includes('pubkey')) return 'hex:02...';
    if (type.includes('bytes') || type.includes('hash') || type.includes('preimage')) return 'hex:00';
    if (type.includes('utf8') || type.includes('string')) return 'utf8:text';
    if (type.includes('bool')) return 'true';
    if (type.includes('int') || type.includes('amount') || type.includes('time') || type.includes('height')) return '0';
    return inputName;
}

function variantHasExplicitInputs(variant) {
    return Boolean(variant && Array.isArray(variant.functionInputs) && variant.functionInputs.length > 0);
}

function buildBindingsTemplate(variant) {
    if (!variantHasExplicitInputs(variant)) {
        return '';
    }

    const lines = variant.functionInputs.map((input) => {
        const name = input?.name || 'arg';
        const value = runtimeBindingDefaultForInput(input?.type, name);
        return `${name}=${value}`;
    });
    return lines.join('\n');
}

function fillBindingsTemplateForSelected(forceReplace = true) {
    const bindingsEl = document.getElementById('runtime-bindings');
    if (!bindingsEl) {
        return false;
    }

    let variant;
    try {
        variant = selectedRuntimeFunction();
    } catch {
        return false;
    }

    if (!variantHasExplicitInputs(variant)) {
        if (bindingsEl.value) {
            bindingsEl.value = '';
            saveRuntimeSettings();
        }
        return true;
    }

    if (!forceReplace && bindingsEl.value.trim()) {
        return false;
    }

    bindingsEl.value = buildBindingsTemplate(variant);
    saveRuntimeSettings();
    return true;
}

function loadRuntimeFunctionOptions(contract) {
    const select = document.getElementById('runtime-function');
    runtimeFunctionVariants = [];
    select.innerHTML = '';

    if (!contract || !Array.isArray(contract.functions) || contract.functions.length === 0) {
        const option = document.createElement('option');
        option.textContent = 'Compile first';
        option.value = '';
        select.appendChild(option);
        select.disabled = true;
        refreshRuntimeActionButtons(false);
        return;
    }

    contract.functions.forEach((func) => {
        runtimeFunctionVariants.push({
            name: func.name,
            serverVariant: Boolean(func.serverVariant),
            functionInputs: Array.isArray(func.functionInputs) ? func.functionInputs : []
        });
        const option = document.createElement('option');
        option.value = String(runtimeFunctionVariants.length - 1);
        option.textContent = runtimeVariantLabel(runtimeFunctionVariants[runtimeFunctionVariants.length - 1]);
        select.appendChild(option);
    });

    select.disabled = false;
    refreshRuntimeActionButtons(false);
}

function parseRuntimeContextJson() {
    const raw = document.getElementById('runtime-context').value || '';
    if (!raw.trim()) {
        return null;
    }
    try {
        const parsed = JSON.parse(raw);
        return JSON.stringify(parsed);
    } catch (err) {
        throw new Error(`Invalid context JSON: ${err.message}`);
    }
}

function applyRuntimeContextPreset() {
    const presetId = document.getElementById('runtime-context-preset').value;
    const contextEl = document.getElementById('runtime-context');
    contextEl.value = RUNTIME_CONTEXT_PRESETS[presetId] ?? '';
    saveRuntimeSettings();
    resetRuntimeUI('Context preset applied. Execute to validate with this fixture.');
}

function runtimeExecutionConfigFromUI() {
    return {
        strictPlaceholders: Boolean(document.getElementById('runtime-strict').checked),
        strictTypes: Boolean(document.getElementById('runtime-strict-types').checked),
        strictBindings: Boolean(document.getElementById('runtime-strict-bindings').checked),
        contextStrict: Boolean(document.getElementById('runtime-context-strict').checked),
        contextJson: parseRuntimeContextJson(),
        mode: document.getElementById('runtime-mode').value || 'development',
    };
}

function executeRuntimeVariant(variant, bindingsJson, options) {
    if (typeof wasmApi.execute_contract_json_advanced === 'function') {
        return wasmApi.execute_contract_json_advanced(
            lastCompiledArtifactJson,
            variant.name,
            variant.serverVariant,
            bindingsJson,
            options.strictPlaceholders,
            options.strictTypes,
            options.strictBindings,
            options.contextJson,
            options.contextStrict,
            options.mode
        );
    }

    if (
        options.strictTypes
        || options.strictBindings
        || options.contextJson
        || options.contextStrict
        || options.mode !== 'development'
    ) {
        console.warn('Advanced runtime controls are unavailable in this WASM build; falling back to base runtime API.');
    }

    if (typeof wasmApi.execute_contract_json === 'function') {
        return wasmApi.execute_contract_json(
            lastCompiledArtifactJson,
            variant.name,
            variant.serverVariant,
            bindingsJson,
            options.strictPlaceholders
        );
    }

    throw new Error('WASM package does not expose runtime API yet. Run ./playground/build.sh and reload.');
}

function runRuntimeVariantWithTiming(variant, bindingsJson, options) {
    const started = performance.now();
    const raw = executeRuntimeVariant(variant, bindingsJson, options);
    const totalMs = performance.now() - started;
    let runtimeJson;
    try {
        runtimeJson = JSON.parse(raw);
    } catch (err) {
        throw new Error(`Runtime returned invalid JSON: ${err.message}`);
    }

    return {
        runtimeJson,
        totalMs,
    };
}

function runtimeOutcomeClass(runtimeJson) {
    if (runtimeJson?.outcome === 'script_true') return 'ok';
    if (runtimeJson?.outcome === 'script_false') return 'fail';
    return 'error';
}

function inferPolicyStepCount(runtimeJson) {
    const counters = runtimeJson?.policy_counters;
    if (counters && typeof counters === 'object') {
        const keys = ['steps', 'policy_steps', 'executed_steps', 'step_count', 'used_steps'];
        for (const key of keys) {
            const value = Number(counters[key]);
            if (Number.isFinite(value)) {
                return value;
            }
        }
    }

    const telemetry = Array.isArray(runtimeJson?.telemetry) ? runtimeJson.telemetry : [];
    return telemetry.reduce((max, step) => {
        const value = Number(step?.policy_steps);
        if (!Number.isFinite(value)) return max;
        return Math.max(max, value);
    }, 0);
}

function matrixStatusCounters(rows) {
    return rows.reduce((acc, row) => {
        if (row.status === 'ok') acc.ok += 1;
        else if (row.status === 'fail') acc.fail += 1;
        else acc.error += 1;
        return acc;
    }, { ok: 0, fail: 0, error: 0 });
}

function beginRuntimeMatrixRender(totalCount) {
    const summaryEl = document.getElementById('runtime-matrix-summary');
    const tableEl = document.getElementById('runtime-matrix-table');
    summaryEl.classList.add('running');
    summaryEl.classList.remove('complete');
    summaryEl.textContent = `Running matrix: 0/${totalCount} paths`;
    tableEl.innerHTML = `<table>
<thead>
<tr>
  <th>Path</th>
  <th>Outcome</th>
  <th>Steps</th>
  <th>VM</th>
  <th>Total</th>
  <th>Error</th>
</tr>
</thead>
<tbody id="runtime-matrix-body"></tbody>
</table>`;
}

function appendRuntimeMatrixRows(rows, startIndex) {
    const body = document.getElementById('runtime-matrix-body');
    if (!body || startIndex >= rows.length) {
        return startIndex;
    }

    const fragment = document.createDocumentFragment();
    for (let idx = startIndex; idx < rows.length; idx++) {
        const row = rows[idx];
        const tr = document.createElement('tr');

        const pathTd = document.createElement('td');
        pathTd.textContent = row.label;
        tr.appendChild(pathTd);

        const outcomeTd = document.createElement('td');
        const outcomePill = document.createElement('span');
        outcomePill.className = `runtime-matrix-pill ${row.status}`;
        outcomePill.textContent = row.outcome;
        outcomeTd.appendChild(outcomePill);
        tr.appendChild(outcomeTd);

        const stepsTd = document.createElement('td');
        stepsTd.textContent = String(row.steps);
        tr.appendChild(stepsTd);

        const vmTd = document.createElement('td');
        vmTd.textContent = row.vmMs;
        tr.appendChild(vmTd);

        const totalTd = document.createElement('td');
        totalTd.textContent = formatMs(row.totalMs);
        tr.appendChild(totalTd);

        const errorTd = document.createElement('td');
        errorTd.textContent = row.error || '-';
        tr.appendChild(errorTd);

        fragment.appendChild(tr);
    }

    body.appendChild(fragment);
    return rows.length;
}

function updateRuntimeMatrixSummary(rows, totalCount, elapsedMs, done = false) {
    const summaryEl = document.getElementById('runtime-matrix-summary');
    summaryEl.classList.toggle('running', !done);
    summaryEl.classList.toggle('complete', done);
    if (!rows.length && done) {
        summaryEl.textContent = 'Run matrix to benchmark every function variant.';
        return;
    }

    const counters = matrixStatusCounters(rows);
    if (done) {
        summaryEl.textContent = `${rows.length} paths in ${formatMs(elapsedMs)} - ${counters.ok} passed, ${counters.fail} script_false, ${counters.error} runtime_error`;
        return;
    }

    summaryEl.textContent = `Running matrix: ${rows.length}/${totalCount} paths in ${formatMs(elapsedMs)} - ${counters.ok} passed, ${counters.fail} script_false, ${counters.error} runtime_error`;
}

function renderRuntimeStep() {
    const stepLabel = document.getElementById('runtime-step-label');
    const stepContainer = document.getElementById('runtime-step');
    const prevBtn = document.getElementById('runtime-prev-step');
    const nextBtn = document.getElementById('runtime-next-step');
    const slider = document.getElementById('runtime-step-slider');

    const entries = runtimeTraceView?.entries || [];
    if (entries.length === 0) {
        runtimeStepIndex = -1;
        stepLabel.textContent = 'Step 0 / 0';
        stepContainer.textContent = '';
        prevBtn.disabled = true;
        nextBtn.disabled = true;
        slider.value = '0';
        slider.max = '0';
        slider.disabled = true;
        return;
    }

    if (runtimeStepIndex < 0) {
        runtimeStepIndex = 0;
    }
    if (runtimeStepIndex >= entries.length) {
        runtimeStepIndex = entries.length - 1;
    }

    const entry = entries[runtimeStepIndex];
    const step = entry.step;
    if ((runtimeTraceView?.stride || 1) > 1) {
        stepLabel.textContent = `Step ${runtimeStepIndex + 1} / ${entries.length} (actual ${entry.sourceIndex + 1} / ${runtimeTraceView.totalSteps})`;
    } else {
        stepLabel.textContent = `Step ${runtimeStepIndex + 1} / ${entries.length}`;
    }
    prevBtn.disabled = runtimeStepIndex === 0;
    nextBtn.disabled = runtimeStepIndex >= entries.length - 1;
    slider.disabled = false;
    slider.max = String(entries.length - 1);
    slider.value = String(runtimeStepIndex);

    let before = runtimeTraceView.beforeCache[runtimeStepIndex];
    if (!before) {
        before = runtimeStackToText(step.stack_before, true);
        runtimeTraceView.beforeCache[runtimeStepIndex] = before;
    }
    let after = runtimeTraceView.afterCache[runtimeStepIndex];
    if (!after) {
        after = runtimeStackToText(step.stack_after, true);
        runtimeTraceView.afterCache[runtimeStepIndex] = after;
    }

    stepContainer.innerHTML = `
<div class="runtime-step-meta">
  <span>ip=${step.ip}</span>
  <span>token=${escapeHtml(step.token)}</span>
  <span>status=${escapeHtml(step.status)}</span>
  <span>elapsed=${formatNanosAsMs(step.elapsed_nanos)}</span>
  <span>policy_steps=${Number(step.policy_steps || 0)}</span>
</div>
<div class="runtime-step-stacks">
  <div>
    <h4>Stack Before</h4>
    <pre>${escapeHtml(before)}</pre>
  </div>
  <div>
    <h4>Stack After</h4>
    <pre>${escapeHtml(after)}</pre>
  </div>
</div>`;
}

function renderRuntimeResult(runtimeJson, totalMs) {
    const summaryEl = document.getElementById('runtime-summary');
    summaryEl.classList.remove('success', 'error', 'stale');
    summaryEl.classList.add(runtimeJson.outcome === 'runtime_error' ? 'error' : 'success');
    markRuntimeFresh();
    runtimeTraceView = buildRuntimeTraceView(runtimeJson);

    const summaryLines = [];
    let outcomeLine = `Outcome: ${runtimeJson.outcome}`;
    if (runtimeJson.error_code) {
        outcomeLine += ` (${runtimeJson.error_code})`;
    }
    summaryLines.push(outcomeLine);
    if (runtimeJson.error_message) {
        summaryLines.push(`Error: ${runtimeJson.error_message}`);
    }
    summaryLines.push(`Function: ${runtimeJson.function_name} | Variant: ${runtimeJson.server_variant ? 'cooperative' : 'exit'}`);
    summaryLines.push(`Compile: ${formatMs(lastCompileDurationMs)} | Total: ${formatMs(totalMs)} | VM: ${formatNanosAsMs(runtimeJson.elapsed_nanos)}`);
    summaryLines.push(`Steps: ${runtimeTraceView.totalSteps}`);
    if (runtimeTraceView.stride > 1) {
        summaryLines.push(`Trace View: sampled every ${runtimeTraceView.stride} steps for UI speed`);
    }
    summaryLines.push(`Trace: ${runtimeJson.trace_version || 'n/a'} / ${runtimeJson.trace_id || 'n/a'}`);
    summaryEl.textContent = summaryLines.join('\n');

    document.getElementById('runtime-main-stack').textContent = runtimeStackToText(runtimeJson.final_main_stack, true);
    document.getElementById('runtime-alt-stack').textContent = runtimeStackToText(runtimeJson.final_alt_stack, true);
    document.getElementById('runtime-metric-total-ms').textContent = formatMs(totalMs);
    document.getElementById('runtime-metric-vm-ms').textContent = formatNanosAsMs(runtimeJson.elapsed_nanos);
    document.getElementById('runtime-metric-steps').textContent = String(runtimeTraceView.totalSteps);
    document.getElementById('runtime-metric-policy').textContent = String(inferPolicyStepCount(runtimeJson));

    document.getElementById('runtime-trace').textContent = JSON.stringify({
        trace_version: runtimeJson.trace_version,
        trace_id: runtimeJson.trace_id,
        seed: runtimeJson.seed ?? null,
        contract_name: runtimeJson.contract_name,
        function_name: runtimeJson.function_name,
        server_variant: runtimeJson.server_variant,
    }, null, 2);

    document.getElementById('runtime-policy').textContent = JSON.stringify({
        runtime_options: runtimeJson.runtime_options ?? {},
        policy_counters: runtimeJson.policy_counters ?? {}
    }, null, 2);

    lastRuntimeResult = runtimeJson;
    runtimeStepIndex = runtimeTraceView.entries.length ? 0 : -1;
    renderRuntimeStep();
}

function showRuntimeError(title, err) {
    lastRuntimeResult = null;
    runtimeStepIndex = -1;
    runtimeTraceView = null;
    markRuntimeFresh();
    const summaryEl = document.getElementById('runtime-summary');
    summaryEl.classList.remove('success', 'stale');
    summaryEl.classList.add('error');
    summaryEl.textContent = `${title}:\n${err instanceof Error ? err.message : String(err)}`;
    document.getElementById('runtime-step').textContent = '';
    document.getElementById('runtime-main-stack').textContent = '';
    document.getElementById('runtime-alt-stack').textContent = '';
    document.getElementById('runtime-step-label').textContent = 'Step 0 / 0';
    document.getElementById('runtime-prev-step').disabled = true;
    document.getElementById('runtime-next-step').disabled = true;
    const slider = document.getElementById('runtime-step-slider');
    slider.value = '0';
    slider.max = '0';
    slider.disabled = true;
    const matrixSummaryEl = document.getElementById('runtime-matrix-summary');
    matrixSummaryEl.classList.remove('running');
    matrixSummaryEl.classList.add('complete');
    setRuntimeMetricDefaults();
}

function selectedRuntimeFunction() {
    const select = document.getElementById('runtime-function');
    if (!select || select.value === '') {
        throw new Error('No runtime function selected');
    }
    const idx = Number.parseInt(select.value, 10);
    if (!Number.isFinite(idx) || idx < 0 || idx >= runtimeFunctionVariants.length) {
        throw new Error('Invalid runtime function selection');
    }
    return runtimeFunctionVariants[idx];
}

// Mark the editor as having uncompiled changes
function markDirty() {
    const btn = document.getElementById('compile-btn');
    btn.classList.remove('compiled', 'needs-compile');
    void btn.offsetWidth;
    btn.classList.add('needs-compile');

    const statusEl = document.getElementById('compile-status');
    statusEl.textContent = '';
    statusEl.className = 'compile-status';

    lastCompileDurationMs = 0;
    lastCompiledArtifactJson = null;
    loadRuntimeFunctionOptions(null);
    resetRuntimeUI('Source changed. Compile again before runtime execution.');
}

// Mark the editor as up-to-date with compiled output
function markCompiled() {
    const btn = document.getElementById('compile-btn');
    btn.classList.remove('needs-compile');
    btn.classList.add('compiled');
}

// Compile the source code
function doCompile() {
    if (!wasmReady || !editor) return false;

    const source = editor.getValue();
    clearErrors();
    const started = performance.now();

    try {
        const result = wasmApi.compile(source);
        const parsed = JSON.parse(result);
        lastCompileDurationMs = performance.now() - started;
        lastCompiledSource = source;
        lastCompiledArtifactJson = result;
        displayJson(result);
        displayAsm(parsed);
        showSuccess(parsed);
        markCompiled();
        loadRuntimeFunctionOptions(parsed);
        fillBindingsTemplateForSelected(false);
        resetRuntimeUI('Compiled successfully. Select a function path and execute.');
        return true;
    } catch (err) {
        lastCompileDurationMs = 0;
        lastCompiledArtifactJson = null;
        loadRuntimeFunctionOptions(null);
        resetRuntimeUI('Compile a contract to enable runtime execution.');
        showError(err.toString());
        return false;
    }
}

// Display JSON output
function displayJson(jsonStr) {
    const container = document.getElementById('json-output');
    if (jsonStr === jsonRenderCacheRaw && jsonRenderCacheHtml) {
        container.innerHTML = jsonRenderCacheHtml;
        return;
    }

    let html;
    if (jsonStr.length > JSON_SYNTAX_HIGHLIGHT_LIMIT) {
        const kb = (jsonStr.length / 1024).toFixed(1);
        html = `<div class="output-render-note">Large artifact (${kb} KB). Syntax highlighting disabled for faster rendering.</div><pre class="plain-json">${escapeHtml(jsonStr)}</pre>`;
    } else {
        html = syntaxHighlightJson(jsonStr);
    }

    jsonRenderCacheRaw = jsonStr;
    jsonRenderCacheHtml = html;
    container.innerHTML = html;
}

// Syntax highlight JSON
function syntaxHighlightJson(json) {
    if (typeof json !== 'string') {
        json = JSON.stringify(json, null, 2);
    }

    return json
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/("(\\u[a-zA-Z0-9]{4}|\\[^u]|[^\\"])*"(\s*:)?|\b(true|false|null)\b|-?\d+(?:\.\d*)?(?:[eE][+\-]?\d+)?)/g, match => {
            let cls = 'json-number';
            if (/^"/.test(match)) {
                if (/:$/.test(match)) {
                    cls = 'json-key';
                    match = match.slice(0, -1); // Remove colon
                    return `<span class="${cls}">${match}</span>:`;
                } else {
                    cls = 'json-string';
                }
            } else if (/true|false/.test(match)) {
                cls = 'json-boolean';
            } else if (/null/.test(match)) {
                cls = 'json-null';
            }
            return `<span class="${cls}">${match}</span>`;
        });
}

// Display Assembly output
function displayAsm(contract) {
    const container = document.getElementById('asm-output');

    try {
        const data = typeof contract === 'string' ? JSON.parse(contract) : contract;
        const pieces = [];

        if (data.functions && data.functions.length > 0) {
            for (const func of data.functions) {
                const variant = func.serverVariant ? 'Cooperative' : 'Exit';
                pieces.push(`<span class="asm-function">${func.name} <span class="asm-variant">(${variant} path)</span></span>`);

                if (func.asm) {
                    const tokenCount = Array.isArray(func.asm)
                        ? func.asm.length
                        : String(func.asm).trim().split(/\s+/).length;
                    const disableHighlight = tokenCount > ASM_TOKEN_HIGHLIGHT_LIMIT;
                    if (disableHighlight) {
                        pieces.push(`<div class="output-render-note">Highlight skipped (${tokenCount} ASM tokens) for faster rendering.</div>`);
                    }
                    pieces.push(highlightAsm(func.asm, disableHighlight));
                }
            }
        } else {
            pieces.push('<span class="comment">No functions compiled</span>');
        }

        container.innerHTML = pieces.join('\n\n');
    } catch (e) {
        container.textContent = 'Failed to parse assembly output';
    }
}

// Highlight assembly code
function highlightAsm(asm, disableHighlight = false) {
    const tokens = Array.isArray(asm)
        ? asm
        : String(asm || '').trim().split(/\s+/).filter(Boolean);

    if (disableHighlight) {
        return `<span class="asm-plain">${escapeHtml(tokens.join(' '))}</span>`;
    }

    return tokens.map(token => {
        const escaped = escapeHtml(token);
        if (token.startsWith('OP_')) {
            return `<span class="asm-opcode">${escaped}</span>`;
        }
        if (token.startsWith('<') && token.endsWith('>')) {
            return `<span class="asm-placeholder">${escaped}</span>`;
        }
        return escaped;
    }).join(' ');
}

function escapeHtml(text) {
    return String(text)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;');
}

function compactMiddle(text, start, end) {
    if (text.length <= start + end + 3) {
        return text;
    }
    return `${text.slice(0, start)}...${text.slice(-end)}`;
}

function formatRuntimeStackValue(value, compact = false) {
    if (!value || typeof value !== 'object') {
        return String(value);
    }

    switch (value.type) {
        case 'int':
            return String(value.value);
        case 'bool':
            return value.value ? 'true' : 'false';
        case 'bytes_hex': {
            const hexValue = String(value.value || '');
            if (compact && hexValue.length > 96) {
                return `0x${compactMiddle(hexValue, 48, 16)} (${Math.floor(hexValue.length / 2)}b)`;
            }
            return `0x${hexValue}`;
        }
        case 'symbol': {
            const symbol = String(value.value || '');
            if (compact && symbol.length > 72) {
                return `<${compactMiddle(symbol, 40, 20)}> (${symbol.length} chars)`;
            }
            return `<${symbol}>`;
        }
        default: {
            const raw = JSON.stringify(value);
            return compact && raw.length > 120 ? `${compactMiddle(raw, 72, 24)} (${raw.length} chars)` : raw;
        }
    }
}

function runtimeStackToText(values, compact = false) {
    if (!Array.isArray(values) || values.length === 0) {
        return '(empty)';
    }
    return values
        .map((value, idx) => `${String(idx).padStart(3, '0')}: ${formatRuntimeStackValue(value, compact)}`)
        .join('\n');
}

function parseRuntimeBindings() {
    const raw = document.getElementById('runtime-bindings').value || '';
    const out = {};
    const lines = raw.split('\n');

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();
        if (!line || line.startsWith('#') || line.startsWith('//')) {
            continue;
        }
        const eq = line.indexOf('=');
        if (eq <= 0) {
            throw new Error(`Invalid binding line ${i + 1}: expected key=value`);
        }
        const key = line.slice(0, eq).trim();
        const rawValue = line.slice(eq + 1).trim();
        if (!key) {
            throw new Error(`Invalid binding line ${i + 1}: missing key`);
        }

        if (rawValue.toLowerCase().startsWith('hex:')) {
            out[key] = { type: 'bytes_hex', value: rawValue.slice(4) };
        } else if (rawValue.toLowerCase().startsWith('utf8:')) {
            out[key] = { type: 'bytes_utf8', value: rawValue.slice(5) };
        } else if (/^(true|false)$/i.test(rawValue)) {
            out[key] = { type: 'bool', value: /^true$/i.test(rawValue) };
        } else if (/^-?\d+$/.test(rawValue)) {
            out[key] = { type: 'int', value: Number.parseInt(rawValue, 10) };
        } else {
            out[key] = { type: 'symbol', value: rawValue };
        }
    }

    return out;
}

function ensureRuntimeArtifactIsFresh() {
    if (!editor) {
        return false;
    }
    if (editor.getValue() !== lastCompiledSource || !lastCompiledArtifactJson) {
        return doCompile();
    }
    return Boolean(lastCompiledArtifactJson);
}

function runRuntime() {
    if (!wasmReady || !ensureRuntimeArtifactIsFresh()) {
        return;
    }

    try {
        cancelRuntimeMatrixRun();
        const fn = selectedRuntimeFunction();
        const bindingsJson = JSON.stringify(parseRuntimeBindings());
        const options = runtimeExecutionConfigFromUI();
        saveRuntimeSettings();
        setRuntimeBusy(true, 'single');
        const { runtimeJson, totalMs } = runRuntimeVariantWithTiming(fn, bindingsJson, options);
        renderRuntimeResult(runtimeJson, totalMs);
        switchTab('runtime');
    } catch (err) {
        showRuntimeError('Runtime execution failed', err);
    } finally {
        setRuntimeBusy(false);
    }
}

async function runRuntimeMatrix() {
    if (!wasmReady || !ensureRuntimeArtifactIsFresh()) {
        return;
    }
    if (!runtimeFunctionVariants.length) {
        showRuntimeError('Runtime matrix failed', new Error('No function variants available'));
        return;
    }

    const runId = ++activeMatrixRunId;
    try {
        const options = runtimeExecutionConfigFromUI();
        const bindingsJson = JSON.stringify(parseRuntimeBindings());
        saveRuntimeSettings();
        setRuntimeBusy(true, 'matrix');

        beginRuntimeMatrixRender(runtimeFunctionVariants.length);
        const matrixStarted = performance.now();
        const rows = [];
        lastRuntimeMatrixResults = [];
        let renderedRows = 0;
        let lastSummaryUpdate = matrixStarted;
        let lastYield = matrixStarted;

        for (let index = 0; index < runtimeFunctionVariants.length; index++) {
            if (runId !== activeMatrixRunId) {
                lastRuntimeMatrixResults = rows.slice();
                return;
            }
            const variant = runtimeFunctionVariants[index];
            try {
                const { runtimeJson, totalMs } = runRuntimeVariantWithTiming(variant, bindingsJson, options);
                rows.push({
                    label: runtimeVariantLabel(variant),
                    outcome: runtimeJson.outcome,
                    status: runtimeOutcomeClass(runtimeJson),
                    steps: runtimeJson.telemetry?.length || 0,
                    vmMs: formatNanosAsMs(runtimeJson.elapsed_nanos),
                    totalMs,
                    error: runtimeJson.error_message || runtimeJson.error_code || '',
                    runtimeJson,
                });
            } catch (err) {
                rows.push({
                    label: runtimeVariantLabel(variant),
                    outcome: 'runtime_error',
                    status: 'error',
                    steps: 0,
                    vmMs: '0.00 ms',
                    totalMs: 0,
                    error: err instanceof Error ? err.message : String(err),
                    runtimeJson: null,
                });
            }

            if ((rows.length - renderedRows) >= MATRIX_RENDER_BATCH_SIZE) {
                renderedRows = appendRuntimeMatrixRows(rows, renderedRows);
                lastRuntimeMatrixResults = rows.slice();
            }

            const now = performance.now();
            if ((now - lastSummaryUpdate) >= MATRIX_PROGRESS_INTERVAL_MS || index === runtimeFunctionVariants.length - 1) {
                updateRuntimeMatrixSummary(rows, runtimeFunctionVariants.length, now - matrixStarted, false);
                lastSummaryUpdate = now;
            }
            if ((now - lastYield) >= MATRIX_YIELD_INTERVAL_MS) {
                await waitForNextFrame();
                lastYield = performance.now();
            }
        }

        renderedRows = appendRuntimeMatrixRows(rows, renderedRows);
        const matrixTotalMs = performance.now() - matrixStarted;
        lastRuntimeMatrixResults = rows.slice();
        updateRuntimeMatrixSummary(rows, runtimeFunctionVariants.length, matrixTotalMs, true);

        if (runId !== activeMatrixRunId) {
            return;
        }

        const selected = selectedRuntimeFunction();
        const focused = rows.find((row) => row.label === runtimeVariantLabel(selected) && row.runtimeJson)
            || rows.find((row) => row.runtimeJson);
        if (focused?.runtimeJson) {
            renderRuntimeResult(focused.runtimeJson, focused.totalMs);
        } else {
            showRuntimeError('Runtime matrix failed', new Error('All variants failed to execute'));
        }
        switchTab('runtime');
    } catch (err) {
        showRuntimeError('Runtime matrix failed', err);
    } finally {
        if (runId === activeMatrixRunId) {
            setRuntimeBusy(false);
        }
    }
}

// Show compilation success
function showSuccess(contract) {
    const statusEl = document.getElementById('compile-status');
    let funcCount = 0;
    try {
        const data = typeof contract === 'string' ? JSON.parse(contract) : contract;
        funcCount = data.functions?.length || 0;
    } catch (e) {
        funcCount = 0;
    }
    statusEl.innerHTML = `<i class="fas fa-check-circle"></i> Compiled &middot; ${funcCount} function${funcCount !== 1 ? 's' : ''} &middot; ${formatMs(lastCompileDurationMs)}`;
    statusEl.className = 'compile-status success';
}

// Show error
function showError(message) {
    const statusEl = document.getElementById('compile-status');
    statusEl.innerHTML = `<i class="fas fa-times-circle"></i> Error`;
    statusEl.className = 'compile-status error';

    const errorsTab = document.getElementById('errors-output');
    const errorCount = document.getElementById('error-count');

    errorsTab.textContent = message;
    errorCount.textContent = '1';
    errorCount.classList.add('visible');

    // Switch to errors tab
    switchTab('errors');

    // Highlight line if possible
    const lineMatch = message.match(/line (\d+)/i);
    if (lineMatch && editor) {
        const lineNumber = parseInt(lineMatch[1], 10);
        editor.revealLineInCenter(lineNumber);
        editor.setSelection({
            startLineNumber: lineNumber,
            startColumn: 1,
            endLineNumber: lineNumber,
            endColumn: 1000
        });
    }
}

// Clear errors
function clearErrors() {
    document.getElementById('errors-output').textContent = '';
    document.getElementById('error-count').textContent = '';
    document.getElementById('error-count').classList.remove('visible');
    const statusEl = document.getElementById('compile-status');
    statusEl.textContent = '';
    statusEl.className = 'compile-status';
}

// Switch output tab
function switchTab(tabName) {
    // Update tab buttons
    document.querySelectorAll('.tab').forEach(tab => {
        tab.classList.toggle('active', tab.dataset.tab === tabName);
    });

    // Update tab content
    document.querySelectorAll('.output-tab').forEach(content => {
        content.classList.toggle('active', content.id === `${tabName}-output`);
    });
}

// Copy to clipboard
async function copyOutput() {
    const activeTab = document.querySelector('.output-tab.active');
    if (!activeTab) return;

    let text = activeTab.textContent;
    if (activeTab.id === 'runtime-output' && lastRuntimeResult) {
        const matrix = lastRuntimeMatrixResults.map((row) => ({
            path: row.label,
            outcome: row.outcome,
            status: row.status,
            steps: row.steps,
            vm: row.vmMs,
            total_ms: Number(row.totalMs.toFixed(2)),
            error: row.error || null,
        }));
        text = JSON.stringify({
            selected_run: lastRuntimeResult,
            matrix,
        }, null, 2);
    }
    try {
        await navigator.clipboard.writeText(text);
        // Visual feedback
        const btn = document.getElementById('copy-btn');
        btn.innerHTML = '<i class="fas fa-check"></i>';
        setTimeout(() => {
            btn.innerHTML = '<i class="fas fa-copy"></i>';
        }, 1500);
    } catch (err) {
        console.error('Failed to copy:', err);
    }
}

// Resizable panels
function initResizer() {
    const divider = document.getElementById('divider');
    const editorPanel = document.querySelector('.editor-panel');
    let isResizing = false;

    divider.addEventListener('mousedown', (e) => {
        isResizing = true;
        divider.classList.add('dragging');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const containerWidth = document.querySelector('main').offsetWidth;
        const newWidth = (e.clientX / containerWidth) * 100;

        if (newWidth > 20 && newWidth < 80) {
            editorPanel.style.flex = `0 0 ${newWidth}%`;
        }
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            divider.classList.remove('dragging');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

// Initialize sidebar resizer
function initSidebarResizer() {
    const divider = document.getElementById('sidebar-divider');
    const sidebar = document.getElementById('sidebar');
    let isResizing = false;

    divider.addEventListener('mousedown', (e) => {
        isResizing = true;
        divider.classList.add('dragging');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const newWidth = e.clientX;
        if (newWidth > 150 && newWidth < 400) {
            sidebar.style.width = `${newWidth}px`;
        }
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            divider.classList.remove('dragging');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    // Load user data from localStorage
    loadFromStorage();

    // Begin decoding URL hash early (async) so it's ready when Monaco is up
    window._urlCodePromise = loadFromUrl();

    // Expand Examples folder by default
    expandedFolders.add('_examples');

    // Render file tree
    renderFileTree();

    // Set initial file state
    currentFile = 'runtime_demo';
    openTabs.push({ id: 'runtime_demo', project: null, file: 'runtime_demo', name: 'RuntimeDemo.ark' });
    fileContents['runtime_demo'] = examples.runtime_demo.code;
    updateFileTabs();

    // Initialize Monaco
    initMonaco();

    // Initialize resizers
    initResizer();
    initSidebarResizer();

    // Tab switching (output tabs)
    document.querySelectorAll('.tab').forEach(tab => {
        tab.addEventListener('click', () => switchTab(tab.dataset.tab));
    });

    // Compile button
    document.getElementById('compile-btn').addEventListener('click', doCompile);
    document.getElementById('runtime-run-btn').addEventListener('click', runRuntime);
    document.getElementById('runtime-run-matrix-btn').addEventListener('click', runRuntimeMatrix);
    document.getElementById('runtime-stop-matrix-btn').addEventListener('click', stopRuntimeMatrix);
    document.getElementById('runtime-prev-step').addEventListener('click', () => {
        if (!lastRuntimeResult) return;
        runtimeStepIndex = Math.max(0, runtimeStepIndex - 1);
        renderRuntimeStep();
    });
    document.getElementById('runtime-next-step').addEventListener('click', () => {
        if (!lastRuntimeResult) return;
        runtimeStepIndex = Math.min(lastRuntimeResult.telemetry.length - 1, runtimeStepIndex + 1);
        renderRuntimeStep();
    });
    document.getElementById('runtime-step-slider').addEventListener('input', (e) => {
        if (!lastRuntimeResult) return;
        const value = Number.parseInt(e.target.value, 10);
        if (!Number.isFinite(value)) return;
        runtimeStepIndex = value;
        renderRuntimeStep();
    });
    document.getElementById('runtime-function').addEventListener('change', () => {
        fillBindingsTemplateForSelected(false);
        invalidateRuntimeResult('Function changed. Execute to refresh runtime results.');
        saveRuntimeSettings();
    });
    document.getElementById('runtime-fill-bindings-btn').addEventListener('click', () => {
        fillBindingsTemplateForSelected(true);
        invalidateRuntimeResult('Bindings template refreshed. Execute to validate.');
    });
    document.getElementById('runtime-apply-context-preset').addEventListener('click', applyRuntimeContextPreset);

    ['runtime-mode', 'runtime-strict', 'runtime-strict-types', 'runtime-strict-bindings', 'runtime-context-strict']
        .forEach((id) => {
            const el = document.getElementById(id);
            el.addEventListener('change', () => {
                saveRuntimeSettings();
                invalidateRuntimeResult('Runtime options changed. Execute again to refresh diagnostics.');
            });
        });

    ['runtime-bindings', 'runtime-context']
        .forEach((id) => {
            const el = document.getElementById(id);
            el.addEventListener('input', () => {
                saveRuntimeSettings();
                invalidateRuntimeResult('Runtime inputs changed. Execute again with updated values.');
            });
        });

    // Cmd/Ctrl+S → compile (prevent browser save dialog)
    document.addEventListener('keydown', (e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === 's') {
            e.preventDefault();
            doCompile();
            return;
        }

        const runtimeTabActive = document.querySelector('.tab.active')?.dataset.tab === 'runtime';
        const activeTag = document.activeElement?.tagName?.toLowerCase();
        const isInputFocused = activeTag === 'textarea' || activeTag === 'input' || activeTag === 'select';
        if (!runtimeTabActive || isInputFocused || !lastRuntimeResult) {
            return;
        }
        if (e.key === 'ArrowLeft') {
            runtimeStepIndex = Math.max(0, runtimeStepIndex - 1);
            renderRuntimeStep();
        } else if (e.key === 'ArrowRight') {
            runtimeStepIndex = Math.min(lastRuntimeResult.telemetry.length - 1, runtimeStepIndex + 1);
            renderRuntimeStep();
        }
    });

    // Copy button
    document.getElementById('copy-btn').addEventListener('click', copyOutput);

    // Share button
    document.getElementById('share-btn').addEventListener('click', shareContract);

    // Sidebar action buttons
    document.getElementById('new-file-btn').addEventListener('click', promptNewStandaloneFile);
    document.getElementById('new-folder-btn').addEventListener('click', promptNewFolder);

    applyRuntimeSettings(loadRuntimeSettings());
    loadRuntimeFunctionOptions(null);
    resetRuntimeUI('Compile a contract to enable runtime execution.');

    // Dismiss context menu on click outside
    document.addEventListener('click', hideContextMenu);
    document.addEventListener('contextmenu', (e) => {
        // Only hide if clicking outside the file tree
        if (!e.target.closest('.file-tree') && !e.target.closest('.context-menu')) {
            hideContextMenu();
        }
    });
});
