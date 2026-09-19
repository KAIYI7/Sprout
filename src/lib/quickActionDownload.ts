/**
 * Save-As flows for Quick Action file Download (ticket 210): one owner for
 * the OS Save-As dialog plus the file writes, shared by the edit dialog's
 * files section and the quick-actions row menu. Single files land as-is,
 * never zipped; the all-files bundle is a frontend-composed stored zip and
 * appears only while the action holds two or more persisted files. Cancelling
 * the dialog aborts silently like every other Save-As flow; anything else
 * throws its plain message for the caller to surface.
 */

import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { t } from "./copy";
import { getQuickActionFile, listQuickActionFiles } from "./api";
import { base64ToBytes, buildStoredZip, downloadFilters } from "./quickActionFiles";

export type DownloadResult = "saved" | "cancelled";

/** Saves one persisted file under its own name through Save-As. */
export async function downloadSingleFile(
  fileId: number,
  filename: string,
): Promise<DownloadResult> {
  const path = await saveDialog({
    title: t("files.downloadOne").replace("{name}", filename),
    defaultPath: filename,
    filters: downloadFilters(filename),
  });
  if (!path) return "cancelled";
  const data = await getQuickActionFile(fileId);
  await writeFile(path, base64ToBytes(data.bytes_base64));
  return "saved";
}

/**
 * Saves every persisted file on the action as one stored zip bundle.
 * Returns `empty` without touching the dialog while fewer than two files
 * persist — the callers gate the affordance on the same threshold, so this
 * is unreachable except through a list racing a concurrent remove.
 */
export async function downloadFilesZip(
  actionId: number,
  actionName: string,
): Promise<DownloadResult | "empty"> {
  const listed = await listQuickActionFiles(actionId);
  if (listed.length < 2) return "empty";
  const path = await saveDialog({
    title: t("files.downloadAll").replace("{name}", actionName),
    defaultPath: `${actionName}-files.zip`,
    filters: [{ name: t("files.zipArchive"), extensions: ["zip"] }],
  });
  if (!path) return "cancelled";
  const blobs = await Promise.all(
    listed.map(async (meta) => {
      const data = await getQuickActionFile(meta.id);
      return { filename: data.filename, bytes: base64ToBytes(data.bytes_base64) };
    }),
  );
  await writeFile(path, buildStoredZip(blobs));
  return "saved";
}
