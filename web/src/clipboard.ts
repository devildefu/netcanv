export async function askForPermission(name: PermissionName): Promise<boolean> {
   // On browsers using Blink, permissions are implemented and we should ask for them first,
   // and if the browser has allowed us, we can use the clipboard.
   //
   // There are permissions on Gecko, but no needed permissions for the clipboard.
   // On WebKit, permissions are not implemented.
   // You can force clipboard with the _FORCE_CLIPBOARD entry in local storage. See above.
   if (navigator.permissions) {
      const permission = await navigator.permissions.query({ name });
      return permission.state === "granted" || permission.state === "prompt";
   } else {
      return false;
   }
}

export function copyString(string: string) {
   navigator.clipboard.writeText(string);
}

export function copyImage(image: Uint8Array) {
   const type = "image/png";
   const blob = new Blob([image], { type });
   const data = [new ClipboardItem({ [type]: blob })];
   navigator.clipboard.write(data);
}

export async function pasteString(): Promise<string> {
   const content = await navigator.clipboard.readText();
   return content;
}

export async function pasteImage(): Promise<Uint8Array | null> {
   const contents = await navigator.clipboard.read();
   for (const item of contents) {
      if (!item.types.includes("image/png")) {
         throw new Error("Clipboard contains non-image data.");
      }
      const blob = await item.getType("image/png");
      const buffer = new Uint8Array(await blob.arrayBuffer());
      return buffer;
   }

   return null;
}
