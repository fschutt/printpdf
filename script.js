import init, {
    Pdf_HtmlToDocument,
    Pdf_BytesToDocument,
    Pdf_PageToSvg,
    Pdf_DocumentToBytes,
    Pdf_ResourcesForPage,
} from './pkg/printpdf.js';

await init(); // Initialize WASM

let pdfDocument = null; // Store the PdfDocument object
let currentTab = 'html-to-pdf';
let currentPageNumber = 1;
let images = {}; // Store uploaded images (filename => base64)
let fonts = {};  // Store uploaded fonts (filename => base64)
let signatureImageBase64 = null;

const actionTabSelect = document.getElementById('action-tab');
const tabContents = {
    'html-to-pdf': document.getElementById('html-to-pdf-tab'),
    'parse-edit-pdf': document.getElementById('parse-edit-pdf-tab'),
    'sign-pdf': document.getElementById('sign-pdf-tab'),
};
const htmlEditorPre = document.getElementById('html-editor');
const htmlLineNumbersDiv = document.querySelector('#html-to-pdf-tab .line-numbers');
const jsonEditorPre = document.getElementById('json-editor');
const jsonLineNumbersDiv = document.querySelector('#parse-edit-pdf-tab .line-numbers');
const pdfViewerDiv = document.getElementById('pdf-viewer');
const pageNumberInput = document.getElementById('page-number');
const prevPageButton = document.getElementById('prev-page');
const nextPageButton = document.getElementById('next-page');
const savePdfButton = document.getElementById('save-pdf');
const minimapViewDiv = document.getElementById('minimap-view');
const sidebarModeButtons = document.querySelectorAll('.sidebar-modes button');
const sidebarContents = {
    'minimap': document.getElementById('minimap-view'),
    'layers': document.getElementById('layers-view'),
    'bookmarks': document.getElementById('bookmarks-view'),
};

const imageUploadInput = document.getElementById('image-upload');
const fontUploadInput = document.getElementById('font-upload');
const pdfFileUploadInput = document.getElementById('pdf-file-upload');
const signatureImageUploadInput = document.getElementById('signature-image-upload');
const pdfViewerErrorText = document.getElementById('pdf-viewer-error');

// Create resource previews container
const htmlResourcesContainer = document.createElement('div');
htmlResourcesContainer.className = 'resources-container';
document.querySelector('#html-to-pdf-tab .controls').appendChild(htmlResourcesContainer);

// Create resource previews container for parse tab
const parseResourcesContainer = document.createElement('div');
parseResourcesContainer.className = 'resources-container';
document.querySelector('#parse-edit-pdf-tab .controls').appendChild(parseResourcesContainer);

// Add save/load config buttons to HTML tab
const configButtonsContainer = document.createElement('div');
configButtonsContainer.className = 'config-buttons-container';
configButtonsContainer.innerHTML = `
    <button id="save-config">Save Configuration</button>
    <button id="load-config">Load Configuration</button>
`;
document.querySelector('#html-to-pdf-tab .controls').prepend(configButtonsContainer);

// Add config file upload input
const configFileInput = document.createElement('input');
configFileInput.type = 'file';
configFileInput.id = 'config-file-upload';
configFileInput.accept = '.json';
configFileInput.style.display = 'none';
document.body.appendChild(configFileInput);

document.getElementById('add-image-html').addEventListener('click', () => imageUploadInput.click());
document.getElementById('add-font-html').addEventListener('click', () => fontUploadInput.click());
document.getElementById('add-image-parse').addEventListener('click', () => imageUploadInput.click());
document.getElementById('add-font-parse').addEventListener('click', () => fontUploadInput.click());
document.getElementById('upload-pdf').addEventListener('click', () => pdfFileUploadInput.click());
document.getElementById('save-config').addEventListener('click', saveConfig);
document.getElementById('load-config').addEventListener('click', () => configFileInput.click());

sidebarModeButtons.forEach(button => {
    button.addEventListener('click', () => {
        sidebarModeButtons.forEach(btn => btn.classList.remove('active'));
        button.classList.add('active');
        const mode = button.dataset.mode;
        Object.values(sidebarContents).forEach(content => content.classList.add('hidden'));
        sidebarContents[mode].classList.remove('hidden');
    });
});

actionTabSelect.addEventListener('change', (event) => {
    currentTab = event.target.value;
    Object.values(tabContents).forEach(content => content.classList.add('hidden'));
    tabContents[currentTab].classList.remove('hidden');
    if (currentTab === 'html-to-pdf') {
        updatePdfFromHtml(); // Initial PDF generation on tab switch if needed
    } else if (currentTab === 'parse-edit-pdf') {
        updateResourcePreviews(parseResourcesContainer, true);
    } else if (currentTab === 'sign-pdf') {
        updatePdfViewer(); // Re-render to show signature if present
    }
});

// Function to base64 encode files
const encodeFileToBase64 = (file, keep_mime) => {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        if (keep_mime === true) {
            reader.onload = () => resolve(reader.result);
        } else {
            reader.onload = () => resolve(reader.result.split(',')[1]); // Remove data:base64 prefix
        }
        reader.onerror = reject;
        reader.readAsDataURL(file);
    });
};

imageUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (file) {
        const base64 = await encodeFileToBase64(file);
        images[file.name] = base64;
        event.target.value = ''; // Reset input
        if (currentTab === 'html-to-pdf') {
            updateResourcePreviews(htmlResourcesContainer);
            updatePdfFromHtml();
        } else if (currentTab === 'parse-edit-pdf') {
            updateResourcePreviews(parseResourcesContainer, true);
            updatePdfFromJsonEditor();
        }
    }
});

fontUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (file) {
        const base64 = await encodeFileToBase64(file);
        fonts[file.name] = base64;
        event.target.value = ''; // Reset input
        if (currentTab === 'html-to-pdf') {
            updateResourcePreviews(htmlResourcesContainer);
            updatePdfFromHtml();
        } else if (currentTab === 'parse-edit-pdf') {
            updateResourcePreviews(parseResourcesContainer, true);
            updatePdfFromJsonEditor();
        }
    }
});

pdfFileUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (!file) return;

    const arrayBuffer = await file.arrayBuffer();
    const base64Pdf = await bufferToBase64(new Uint8Array(arrayBuffer));

    try {
        const inputParse = { pdfBase64: base64Pdf, options: {} };
        const inputParseJson = JSON.stringify(inputParse);
        const parseResultJson = await Pdf_BytesToDocument(inputParseJson);
        const parseResult = JSON.parse(parseResultJson);

        if (parseResult.status === 0) {
            pdfDocument = parseResult.data.pdf;
            for (let i = 0; i < parseResult.data.warnings.length; i++) {
                console.warn(parseResult.data.warnings[i]);
            }
            jsonEditorPre.textContent = JSON.stringify(pdfDocument, null, 2); // Display JSON in editor
            updateLineNumbers(jsonEditorPre, jsonLineNumbersDiv);
            updateResourcePreviews(parseResourcesContainer, true);
            updatePdfViewer();
        } else {
            alert2("PDF Parsing Error: " + parseResult.data);
        }
    } catch (error) {
        alert2("Error parsing PDF: " + error);
    } finally {
        event.target.value = ''; // Reset input
    }
});

configFileInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (!file) return;
    
    try {
        const text = await file.text();
        const config = JSON.parse(text);
        
        // Load saved configuration
        if (config.images) {
            images = config.images;
        }
        if (config.fonts) {
            fonts = config.fonts;
        }
        if (config.html) {
            htmlEditorPre.textContent = config.html;
            updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
        }
        
        updateResourcePreviews(htmlResourcesContainer);
        await updatePdfFromHtml();
    } catch (error) {
        alert2("Error loading configuration: " + error);
    } finally {
        event.target.value = ''; // Reset input
    }
});

signatureImageUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (file) {
        signatureImageBase64 = await encodeFileToBase64(file, true);
        // Update PDF viewer immediately to preview signature
        await updatePdfViewer();
    }
});

// Update resource previews
function updateResourcePreviews(container, isParseTab = false) {
    container.innerHTML = '';
    
    // Create scrollable container
    const scrollContainer = document.createElement('div');
    scrollContainer.className = 'resources-scroll-container';
    
    // Add images section
    if (Object.keys(images).length > 0) {
        const imagesSection = document.createElement('div');
        imagesSection.className = 'resources-section';
        imagesSection.innerHTML = '<h4>Images</h4>';
        const imagesGrid = document.createElement('div');
        imagesGrid.className = 'resources-grid';
        
        Object.entries(images).forEach(([name, base64]) => {
            const resourceItem = createResourceItem(name, `data:image/png;base64,${base64}`, 'image', isParseTab);
            imagesGrid.appendChild(resourceItem);
        });
        
        imagesSection.appendChild(imagesGrid);
        scrollContainer.appendChild(imagesSection);
    }
    
    // Add fonts section
    if (Object.keys(fonts).length > 0) {
        const fontsSection = document.createElement('div');
        fontsSection.className = 'resources-section';
        fontsSection.innerHTML = '<h4>Fonts</h4>';
        const fontsGrid = document.createElement('div');
        fontsGrid.className = 'resources-grid';
        
        Object.entries(fonts).forEach(([name, base64]) => {
            const resourceItem = createResourceItem(name, null, 'font', isParseTab);
            fontsGrid.appendChild(resourceItem);
        });
        
        fontsSection.appendChild(fontsGrid);
        scrollContainer.appendChild(fontsSection);
    }
    
    container.appendChild(scrollContainer);
}

function createResourceItem(name, src, type, isReadOnly = false) {
    const resourceItem = document.createElement('div');
    resourceItem.className = 'resource-item';
    
    // Preview box
    const preview = document.createElement('div');
    preview.className = 'resource-preview';
    
    if (type === 'image' && src) {
        const img = document.createElement('img');
        img.src = src;
        img.alt = name;
        preview.appendChild(img);
    } else if (type === 'font') {
        preview.textContent = 'Aa';
        preview.className += ' font-preview';
    }
    
    // Name with truncation
    const nameEl = document.createElement('div');
    nameEl.className = 'resource-name';
    nameEl.title = name; // Full name on hover
    nameEl.textContent = name.length > 12 ? name.substring(0, 9) + '...' : name;
    
    // Delete button (only for non-readonly)
    if (!isReadOnly) {
        const deleteBtn = document.createElement('button');
        deleteBtn.className = 'resource-delete';
        deleteBtn.innerHTML = '×';
        deleteBtn.title = 'Remove resource';
        deleteBtn.addEventListener('click', async () => {
            if (type === 'image') {
                delete images[name];
            } else if (type === 'font') {
                delete fonts[name];
            }
            
            // Update previews and re-render PDF
            if (currentTab === 'html-to-pdf') {
                updateResourcePreviews(htmlResourcesContainer);
                await updatePdfFromHtml();
            } else if (currentTab === 'parse-edit-pdf') {
                updateResourcePreviews(parseResourcesContainer, true);
                await updatePdfFromJsonEditor();
            }
        });
        resourceItem.appendChild(deleteBtn);
    }
    
    resourceItem.appendChild(preview);
    resourceItem.appendChild(nameEl);
    
    return resourceItem;
}

// Function to update PDF viewer with SVGs from pdfDocument
async function updatePdfViewer() {
    if (!pdfDocument) return;

    pdfViewerDiv.innerHTML = ''; // Clear existing viewer
    minimapViewDiv.innerHTML = ''; // Clear minimap

    for (let i = 0; i < pdfDocument.pages.length; i++) {
        const page = pdfDocument.pages[i];
        try {
            const resourcesInput = JSON.stringify({ page: page });
            const resourcesJson = await Pdf_ResourcesForPage(resourcesInput);
            const resourcesResult = JSON.parse(resourcesJson);

            if (resourcesResult.status !== 0) {
                console.error("Error getting resources for page:", resourcesResult.data);
                continue; // Skip page rendering if resources fail
            }

            let res = pdfDocument.resources; // Use document resources directly
            
            // Apply signature if in "Sign PDF" tab and on correct page
            let modifiedPage = page;
            if (signatureImageBase64 != null && currentTab === 'sign-pdf' && (i + 1) === parseInt(document.getElementById('signature-page').value)) {
                const applied = applySignatureToPage(page, pdfDocument.resources);
                modifiedPage = applied.data;
                res = applied.resources;
                resourcesResult.data.xobjects.push('user-signature-image');
            }

            const svgI = { 
                page: modifiedPage, 
                resources: copyResourcesForPage(res, resourcesResult.data), 
                options: { image_formats: ["png", "jpeg"] }
            };
            const svgInput = JSON.stringify(svgI);
            const svgJson = await Pdf_PageToSvg(svgInput);
            const svgResult = JSON.parse(svgJson);

            if (svgResult.status === 0) {
                const svgString = svgResult.data.svg;
                pdfViewerDiv.innerHTML += svgString;
                minimapViewDiv.innerHTML += svgString;
                pdfViewerErrorText.innerHTML = "";
            } else {
                console.error("Error rendering page to SVG:", svgResult.data);
                pdfViewerDiv.innerHTML += `<p class="error">Error rendering page ${i + 1}: ${svgResult.data}</p>`;
            }
        } catch (error) {
            console.error("Error processing page:", error);
            pdfViewerDiv.innerHTML += `<p class="error">Error processing page ${i + 1}: ${error}</p>`;
        }
    }
    updatePageNavigation();
}

/// Returns a new PdfResources object, but with only the resources needed for the page
function copyResourcesForPage(resources, resourcesResult) {
    let newResources = {
        fonts: {},
        xobjects: {},
        layers: {},
        extgstates: resources.extgstates,
    };

    for (let i = 0; i < resourcesResult.xobjects.length; i++) {
        const id = resourcesResult.xobjects[i];
        newResources.xobjects[id] = resources.xobjects[id];
    }

    for (let i = 0; i < resourcesResult.layers.length; i++) {
        const id = resourcesResult.layers[i];
        newResources.layers[id] = resources.layers[id];
    }

    for (let i = 0; i < resourcesResult.fonts.length; i++) {
        const id = resourcesResult.fonts[i];
        newResources.fonts[id] = resources.fonts[id];
    }

    return newResources;
}

function applySignatureToPage(page, resources) {
    if (!signatureImageBase64) return { data: page, resources: resources };
    
    const signatureImageId = 'user-signature-image';
    const res2 = {
        ...resources,
        xobjects: {
            ...resources.xobjects,
            'user-signature-image': { type: 'image', data: signatureImageBase64 },
        }
    };
    
    const signatureX = parseFloat(document.getElementById('signature-x').value);
    const signatureY = parseFloat(document.getElementById('signature-y').value);
    const signatureScaleX = parseFloat(document.getElementById('signature-scale-x').value);
    const signatureScaleY = parseFloat(document.getElementById('signature-scale-y').value);

    const newOps = [...page.ops, {
        type: "use-xobject",
        data: {
            id: signatureImageId,
            transform: {
                translateX: signatureX,
                translateY: signatureY,
                scaleX: signatureScaleX,
                scaleY: signatureScaleY,
                rotate: null,
                dpi: null
            }
        }
    }];

    return {
        resources: res2,
        data: { ...page, ops: newOps }
    };
}

// Function to update PDF from HTML editor content
async function updatePdfFromHtml() {
    const htmlContent = htmlEditorPre.textContent;

    // Extract the title, width, height, etc. from HTML content
    // In a real implementation, these would be parsed from the HTML or CSS
    const generationOptions = {
        pageWidth: 210,
        pageHeight: 297,
        imageCompression: null,
        fontEmbedding: true
    };

    const input = {
        title: "PDF Document",
        html: htmlContent,
        images: images,
        fonts: fonts,
        options: generationOptions
    };

    const inputJson = JSON.stringify(input);

    try {
        const resultJson = await Pdf_HtmlToDocument(inputJson);
        const result = JSON.parse(resultJson);

        if (result.status === 0) {
            pdfDocument = result.data;
            await updatePdfViewer();
        } else {
            alert2("PDF Generation Error: " + result.data);
        }
    } catch (error) {
        alert2("Error generating PDF: " + error);
    }
}

function alert2(s) {
    pdfViewerErrorText.innerHTML = "<p>" + s + "</p>";
}

// Save configuration to JSON file
function saveConfig() {
    const config = {
        html: htmlEditorPre.textContent,
        images: images,
        fonts: fonts
    };
    
    const blob = new Blob([JSON.stringify(config, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'pdf-config.json';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

// Event listener for HTML editor changes (throttled)
let htmlEditorTimeout;
htmlEditorPre.addEventListener('input', () => {
    clearTimeout(htmlEditorTimeout);
    htmlEditorTimeout = setTimeout(() => updatePdfFromHtml(), 500); // 500ms delay
    updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
});
htmlEditorPre.addEventListener('scroll', () => {
    htmlLineNumbersDiv.scrollTop = htmlEditorPre.scrollTop;
});
htmlEditorPre.addEventListener('keydown', (event) => {
    if (event.key === 'Tab') { // Handle Tab key for indentation
        event.preventDefault();
        document.execCommand('insertText', false, '\t');
    }
});

// Event listener for JSON editor changes (throttled)
let jsonEditorTimeout;
jsonEditorPre.addEventListener('input', () => {
    clearTimeout(jsonEditorTimeout);
    jsonEditorTimeout = setTimeout(() => updatePdfFromJsonEditor(), 1000); // 1 sec delay
    updateLineNumbers(jsonEditorPre, jsonLineNumbersDiv);
});
jsonEditorPre.addEventListener('scroll', () => {
    jsonLineNumbersDiv.scrollTop = jsonEditorPre.scrollTop;
});
jsonEditorPre.addEventListener('keydown', (event) => {
    if (event.key === 'Tab') { // Handle Tab key for indentation in JSON editor
        event.preventDefault();
        document.execCommand('insertText', false, '\t');
    }
});

async function updatePdfFromJsonEditor() {
    try {
        pdfDocument = JSON.parse(jsonEditorPre.textContent);
        await updatePdfViewer();
    } catch (e) {
        alert2("JSON Parse Error: " + e.message);
    }
}

// PDF Viewer Navigation
prevPageButton.addEventListener('click', () => {
    currentPageNumber = Math.max(1, currentPageNumber - 1);
    updatePageNavigation();
});

nextPageButton.addEventListener('click', () => {
    if (pdfDocument) {
        currentPageNumber = Math.min(pdfDocument.pages.length, currentPageNumber + 1);
        updatePageNavigation();
    }
});

pageNumberInput.addEventListener('change', () => {
    currentPageNumber = Math.max(1, Math.min(pdfDocument ? pdfDocument.pages.length : 1, parseInt(pageNumberInput.value) || 1));
    updatePageNavigation();
});

function updatePageNavigation() {
    pageNumberInput.value = currentPageNumber;
    const pages = pdfViewerDiv.querySelectorAll('svg');
    if (pages.length > 0 && currentPageNumber >= 1 && currentPageNumber <= pages.length) {
        pages[currentPageNumber - 1].scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
}

savePdfButton.addEventListener('click', async () => {
    if (!pdfDocument) {
        alert2("No PDF document to save.");
        return;
    }

    try {
        // Apply signature permanently to PDF if in sign mode
        if (currentTab === 'sign-pdf' && signatureImageBase64) {
            const signaturePage = parseInt(document.getElementById('signature-page').value) - 1;
            if (signaturePage >= 0 && signaturePage < pdfDocument.pages.length) {
                const applied = applySignatureToPage(pdfDocument.pages[signaturePage], pdfDocument.resources);
                
                // Update the signature in the document
                pdfDocument.pages[signaturePage] = applied.data;
                pdfDocument.resources = applied.resources;
            }
        }

        const inputBytes = { pdf: pdfDocument, options: {} };
        const inputBytesJson = JSON.stringify(inputBytes);
        const bytesResultJson = await Pdf_DocumentToBytes(inputBytesJson);
        const bytesResult = JSON.parse(bytesResultJson);

        if (bytesResult.status === 0) {
            const base64Pdf = bytesResult.data.pdfBase64;
            const pdfBytes = base64ToUint8Array(base64Pdf);
            downloadPdf(pdfBytes, "document");
        } else {
            alert2("PDF Serialization Error: " + bytesResult.data);
        }
    } catch (error) {
        alert2("Error saving PDF: " + error);
    }
});

function base64ToUint8Array(base64) {
    const binaryString = atob(base64);
    const byteArray = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
        byteArray[i] = binaryString.charCodeAt(i);
    }
    return byteArray;
}

// note: `buffer` arg can be an ArrayBuffer or a Uint8Array
// await bufferToBase64(new Uint8Array([1,2,3,100,200]))
async function bufferToBase64(buffer) {
    // use a FileReader to generate a base64 data URI:
    const base64url = await new Promise(r => {
      const reader = new FileReader()
      reader.onload = () => r(reader.result)
      reader.readAsDataURL(new Blob([buffer]))
    });
    // remove the `data:...;base64,` part from the start
    return base64url.slice(base64url.indexOf(',') + 1);
}

function downloadPdf(pdfBytes, filename) {
    const blob = new Blob([pdfBytes], { type: 'application/pdf' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename + '.pdf';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

function updateLineNumbers(editorElement, lineNumberDiv) {
    const lines = editorElement.textContent.split('\n').length;
    let numbers = '';
    for (let i = 1; i <= lines; i++) {
        numbers += i + '\n';
    }
    lineNumberDiv.textContent = numbers;
}

// Initial setup:
updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
updateLineNumbers(jsonEditorPre, jsonLineNumbersDiv);
updatePdfFromHtml(); // Generate initial PDF on load