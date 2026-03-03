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

// ── localStorage persistence ──────────────────────────────────────
const STORAGE_KEY = 'arkade-playground';

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
            value: examples.single_sig.code,
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

function resetRuntimeUI(message) {
    lastRuntimeResult = null;
    runtimeStepIndex = -1;

    const summaryEl = document.getElementById('runtime-summary');
    summaryEl.classList.remove('success', 'error');
    summaryEl.textContent = message;

    document.getElementById('runtime-step-label').textContent = 'Step 0 / 0';
    document.getElementById('runtime-step').textContent = '';
    document.getElementById('runtime-main-stack').textContent = '';
    document.getElementById('runtime-alt-stack').textContent = '';
    document.getElementById('runtime-prev-step').disabled = true;
    document.getElementById('runtime-next-step').disabled = true;
}

function loadRuntimeFunctionOptions(contract) {
    const select = document.getElementById('runtime-function');
    const runBtn = document.getElementById('runtime-run-btn');
    runtimeFunctionVariants = [];
    select.innerHTML = '';

    if (!contract || !Array.isArray(contract.functions) || contract.functions.length === 0) {
        const option = document.createElement('option');
        option.textContent = 'Compile first';
        option.value = '';
        select.appendChild(option);
        select.disabled = true;
        runBtn.disabled = true;
        return;
    }

    contract.functions.forEach((func) => {
        runtimeFunctionVariants.push({
            name: func.name,
            serverVariant: Boolean(func.serverVariant)
        });
        const option = document.createElement('option');
        const variant = func.serverVariant ? 'cooperative' : 'exit';
        option.value = String(runtimeFunctionVariants.length - 1);
        option.textContent = `${func.name} (${variant})`;
        select.appendChild(option);
    });

    select.disabled = false;
    runBtn.disabled = false;
}

// Mark the editor as having uncompiled changes
function markDirty() {
    const btn = document.getElementById('compile-btn');
    // Re-trigger animation by removing and re-adding the class
    btn.classList.remove('compiled', 'needs-compile');
    void btn.offsetWidth; // reflow to restart animation
    btn.classList.add('needs-compile');

    const statusEl = document.getElementById('compile-status');
    statusEl.textContent = '';
    statusEl.className = 'compile-status';

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
    if (!wasmReady || !editor) return;

    const source = editor.getValue();
    clearErrors();

    try {
        const result = wasmApi.compile(source);
        const parsed = JSON.parse(result);
        lastCompiledSource = source;
        lastCompiledArtifactJson = result;
        displayJson(result);
        displayAsm(result);
        showSuccess(result);
        markCompiled();
        loadRuntimeFunctionOptions(parsed);
        resetRuntimeUI('Compiled successfully. Select a function path and execute.');
    } catch (err) {
        lastCompiledArtifactJson = null;
        loadRuntimeFunctionOptions(null);
        resetRuntimeUI('Compile a contract to enable runtime execution.');
        showError(err.toString());
    }
}

// Display JSON output
function displayJson(jsonStr) {
    const container = document.getElementById('json-output');
    container.innerHTML = syntaxHighlightJson(jsonStr);
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
function displayAsm(jsonStr) {
    const container = document.getElementById('asm-output');

    try {
        const data = JSON.parse(jsonStr);
        let html = '';

        if (data.functions && data.functions.length > 0) {
            for (const func of data.functions) {
                const variant = func.serverVariant ? 'Cooperative' : 'Exit';
                html += `<span class="asm-function">${func.name} <span class="asm-variant">(${variant} path)</span></span>\n`;

                if (func.asm) {
                    html += highlightAsm(func.asm) + '\n\n';
                }
            }
        } else {
            html = '<span class="comment">No functions compiled</span>';
        }

        container.innerHTML = html;
    } catch (e) {
        container.textContent = 'Failed to parse assembly output';
    }
}

// Highlight assembly code
function highlightAsm(asm) {
    const tokens = Array.isArray(asm) ? asm : asm.split(' ');
    return tokens
        .map(token => {
            if (token.startsWith('OP_')) {
                return `<span class="asm-opcode">${token}</span>`;
            } else if (token.startsWith('<') && token.endsWith('>')) {
                return `<span class="asm-placeholder">${token}</span>`;
            }
            return token;
        })
        .join(' ');
}

function escapeHtml(text) {
    return String(text)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;');
}

function formatRuntimeStackValue(value) {
    if (!value || typeof value !== 'object') {
        return String(value);
    }

    switch (value.type) {
        case 'int':
            return String(value.value);
        case 'bool':
            return value.value ? 'true' : 'false';
        case 'bytes_hex':
            return `0x${value.value}`;
        case 'symbol':
            return `<${value.value}>`;
        default:
            return JSON.stringify(value);
    }
}

function runtimeStackToText(values) {
    if (!Array.isArray(values) || values.length === 0) {
        return '(empty)';
    }
    return values
        .map((value, idx) => `${String(idx).padStart(3, '0')}: ${formatRuntimeStackValue(value)}`)
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

function renderRuntimeStep() {
    const stepLabel = document.getElementById('runtime-step-label');
    const stepContainer = document.getElementById('runtime-step');
    const prevBtn = document.getElementById('runtime-prev-step');
    const nextBtn = document.getElementById('runtime-next-step');

    const telemetry = lastRuntimeResult?.telemetry || [];
    if (telemetry.length === 0) {
        runtimeStepIndex = -1;
        stepLabel.textContent = 'Step 0 / 0';
        stepContainer.textContent = '';
        prevBtn.disabled = true;
        nextBtn.disabled = true;
        return;
    }

    if (runtimeStepIndex < 0) {
        runtimeStepIndex = 0;
    }
    if (runtimeStepIndex >= telemetry.length) {
        runtimeStepIndex = telemetry.length - 1;
    }

    const step = telemetry[runtimeStepIndex];
    stepLabel.textContent = `Step ${runtimeStepIndex + 1} / ${telemetry.length}`;
    prevBtn.disabled = runtimeStepIndex === 0;
    nextBtn.disabled = runtimeStepIndex >= telemetry.length - 1;

    const before = runtimeStackToText(step.stack_before);
    const after = runtimeStackToText(step.stack_after);

    stepContainer.innerHTML = `
<div class="runtime-step-meta">
  <span>ip=${step.ip}</span>
  <span>token=${escapeHtml(step.token)}</span>
  <span>status=${step.status}</span>
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

function renderRuntimeResult(runtimeJson) {
    const summaryEl = document.getElementById('runtime-summary');
    summaryEl.classList.remove('success', 'error');
    summaryEl.classList.add(runtimeJson.outcome === 'runtime_error' ? 'error' : 'success');

    let summary = `Outcome: ${runtimeJson.outcome}`;
    if (runtimeJson.error_code) {
        summary += ` (${runtimeJson.error_code})`;
    }
    if (runtimeJson.error_message) {
        summary += `\n${runtimeJson.error_message}`;
    }
    summary += `\nFunction: ${runtimeJson.function_name} | Variant: ${runtimeJson.server_variant ? 'cooperative' : 'exit'}`;
    summary += `\nSteps: ${runtimeJson.telemetry?.length || 0}`;
    summaryEl.textContent = summary;

    document.getElementById('runtime-main-stack').textContent = runtimeStackToText(runtimeJson.final_main_stack);
    document.getElementById('runtime-alt-stack').textContent = runtimeStackToText(runtimeJson.final_alt_stack);

    lastRuntimeResult = runtimeJson;
    runtimeStepIndex = runtimeJson.telemetry?.length ? 0 : -1;
    renderRuntimeStep();
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

function ensureRuntimeArtifactIsFresh() {
    if (!editor) {
        return false;
    }
    if (editor.getValue() !== lastCompiledSource || !lastCompiledArtifactJson) {
        doCompile();
    }
    return Boolean(lastCompiledArtifactJson);
}

function runRuntime() {
    if (!wasmReady || !ensureRuntimeArtifactIsFresh()) {
        return;
    }

    try {
        const fn = selectedRuntimeFunction();
        const strict = document.getElementById('runtime-strict').checked;
        const bindings = parseRuntimeBindings();
        if (typeof wasmApi.execute_contract_json !== 'function') {
            throw new Error(
                'WASM package does not expose runtime API yet. Run ./playground/build.sh and reload.'
            );
        }
        const runtimeJson = wasmApi.execute_contract_json(
            lastCompiledArtifactJson,
            fn.name,
            fn.serverVariant,
            JSON.stringify(bindings),
            strict,
            ''
        );
        renderRuntimeResult(JSON.parse(runtimeJson));
        switchTab('runtime');
    } catch (err) {
        const summaryEl = document.getElementById('runtime-summary');
        summaryEl.classList.remove('success');
        summaryEl.classList.add('error');
        summaryEl.textContent = `Runtime execution failed:\n${err}`;
    }
}

// Show compilation success
function showSuccess(jsonStr) {
    const statusEl = document.getElementById('compile-status');
    let funcCount = '';
    try {
        const data = JSON.parse(jsonStr);
        const count = data.functions?.length || 0;
        funcCount = ` &mdash; ${count} function${count !== 1 ? 's' : ''}`;
    } catch (e) {}
    statusEl.innerHTML = `<i class="fas fa-check-circle"></i> Compiled${funcCount}`;
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
        text = JSON.stringify(lastRuntimeResult, null, 2);
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
    currentFile = 'single_sig';
    openTabs.push({ id: 'single_sig', project: null, file: 'single_sig', name: 'SingleSig.ark' });
    fileContents['single_sig'] = examples.single_sig.code;
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
    document.getElementById('runtime-function').addEventListener('change', () => {
        if (!lastRuntimeResult) return;
        resetRuntimeUI('Function changed. Execute to refresh runtime results.');
    });

    // Cmd/Ctrl+S → compile (prevent browser save dialog)
    document.addEventListener('keydown', (e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === 's') {
            e.preventDefault();
            doCompile();
        }
    });

    // Copy button
    document.getElementById('copy-btn').addEventListener('click', copyOutput);

    // Share button
    document.getElementById('share-btn').addEventListener('click', shareContract);

    // Sidebar action buttons
    document.getElementById('new-file-btn').addEventListener('click', promptNewStandaloneFile);
    document.getElementById('new-folder-btn').addEventListener('click', promptNewFolder);

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
