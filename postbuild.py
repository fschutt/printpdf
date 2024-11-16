import sys, base64

def is_prod():
    l = len(sys.argv)
    if l > 1:
        return sys.argv[1] == "--production"
    else:
        return False
    
def read_file(path):
    text_file = open(path, 'r')
    text_file_contents = text_file.read()
    text_file.close()
    return text_file_contents

def read_file_base64(path):
    encoded_string = ""
    with open(path, "rb") as image_file:
        encoded_string = base64.b64encode(image_file.read()).decode()
    return encoded_string

def write_file(string, path):
    text_file = open(path, "w+", newline='')
    text_file.write(string)
    text_file.close()

def chunks(lst, n):
    """Yield successive n-sized chunks from lst."""
    for i in range(0, len(lst), n):
        yield lst[i:i + n]

def fixup_js_bindings(bindings):
    bindings_fixed = []
    emit_wr = True
    fixup_js = "\r\n".join([
        "async function __wbg_init(input) {",
        "    if (wasm !== undefined) return wasm;",
        "    const imports = __wbg_get_imports();",
        "    __wbg_init_memory(imports);",
        "    var v = base64ToArrayBuffer(window.GLOBAL_WASM);",
        "    const { instance, module } = await WebAssembly.instantiate(v, imports);",
        "    return __wbg_finalize_init(instance, module);",
        "}",
    ])
    for line in bindings.splitlines():
        if "async function __wbg_init(" in line:
            emit_wr = False
            for l in fixup_js.splitlines():
                bindings_fixed.append(l)
        elif "export { initSync }" in line:
            emit_wr = True
            bindings_fixed.append(line)
        else:
            if emit_wr:
                bindings_fixed.append(line)
            else:
                pass

    bindings_fixed.append("")
    return "\r\n".join(bindings_fixed)

def format_wasm_file(b64):
    global_wasm_script = chunks(b64, 100)
    wasm_script = ["window.GLOBAL_WASM = ["]
    for l in global_wasm_script:
        wasm_script.append("    \"" + l + "\",")
    wasm_script_out = "\r\n".join(wasm_script)
    wasm_script_out += "\r\n].join('');\r\n"
    return wasm_script_out

# unzip web/pdfjs

import zipfile
with zipfile.ZipFile("web/pdfjs-4.7.76-legacy-dist.zip","r") as zip_ref:
    zip_ref.extractall("web")

index_html = read_file("./skeleton.html")
index_html = index_html.replace("$$HELLOWORLD_XML$$", read_file("./web/helloworld.xml.txt"))
index_html = index_html.replace("$$CHURCHBOOKLET_XML$$", read_file("./web/churchbooklet.xml.txt"))
index_html = index_html.replace("$$RECIPE_XML$$", read_file("./web/recipe-japanese.xml.txt"))

build_mjs = read_file("./web/pdfjs-4.7.76-legacy-dist/build/pdf.mjs")
viewer_mjs = read_file("./web/pdfjs-4.7.76-legacy-dist/web/viewer.mjs")
pkg_viewer_wasm = ""
is_production = is_prod();
if is_production:
    pkg_viewer_wasm = format_wasm_file(read_file_base64("./pkg/printpdf_bg.wasm"))
pkg_viewer_js = ""
if is_production:
    pkg_viewer_js = fixup_js_bindings(read_file("./pkg/printpdf.js"))

out_file = []
for line in index_html.splitlines():
    if "// PUT_WASM_JS_HERE" in line:
        out_file.append(pkg_viewer_wasm)
        out_file.append(pkg_viewer_js)
    elif "// PUT_BUILD_MJS_HERE" in line:
        out_file.append(build_mjs)
    elif "// PUT_PDF_VIEWER_JS_HERE" in line:
        out_file.append(viewer_mjs)
    elif "var is_prod = false;" in line:
        if is_production:
            out_file.append("var is_prod = true;")
        else:
            out_file.append("var is_prod = false;")
    else:
        out_file.append(line)

write_file("\r\n".join(out_file), "index.html")