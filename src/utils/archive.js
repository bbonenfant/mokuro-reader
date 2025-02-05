// import "https://cdnjs.cloudflare.com/ajax/libs/dexie/4.0.10/dexie.min.js";
import "https://cdnjs.cloudflare.com/ajax/libs/jszip/3.10.1/jszip.min.js";


export class Archive {
    constructor() {
        this.zip = new JSZip();
    }

    async load(file) {
        await this.zip.loadAsync(file);
    }

    async generate() {
        const options = {type: "uint8array"}
        return await this.zip.generateAsync(options);
    }

    async get(name) {
        return this.zip.file(name).async("uint8array");
    }

    async set(name, data) {
        const options = {
            binary: true,
            compression: "DEFLATE",
            compressionOptions: {level: 9},
            createFolders: false
        }
        this.zip.file(name, data, options)
    }

    add_directory(name) {
        this.zip.folder(name);
    }
}
