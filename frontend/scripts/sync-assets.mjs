import { cp, mkdir, rm, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const frontendRoot = path.resolve(__dirname, "..");
const sourceAssetsDir = path.resolve(frontendRoot, "../assets");
const targetAssetsDir = path.resolve(frontendRoot, "public/assets");

async function pathExists(targetPath) {
    try {
        await stat(targetPath);
        return true;
    } catch {
        return false;
    }
}

async function syncAssets() {
    const hasSourceAssets = await pathExists(sourceAssetsDir);

    if (!hasSourceAssets) {
        console.warn(`[sync-assets] Skipped: source directory not found at ${sourceAssetsDir}`);
        return;
    }

    await mkdir(path.dirname(targetAssetsDir), { recursive: true });
    await rm(targetAssetsDir, { recursive: true, force: true });
    await cp(sourceAssetsDir, targetAssetsDir, { recursive: true, force: true });

    console.log(`[sync-assets] Synced ${sourceAssetsDir} -> ${targetAssetsDir}`);
}

syncAssets().catch((err) => {
    console.error("[sync-assets] Failed to sync assets", err);
    process.exitCode = 1;
});
