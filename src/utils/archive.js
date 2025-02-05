// import "https://cdnjs.cloudflare.com/ajax/libs/dexie/4.0.10/dexie.min.js";
import "https://cdnjs.cloudflare.com/ajax/libs/jszip/3.10.1/jszip.min.js";


export class Archive {
    constructor() {
        this.zip = undefined;
    }

    async load(file) {
        this.zip = await JSZip.loadAsync(file);
    }

    async file(name) {
        return this.zip.file(name).async("uint8array");
    }
}
