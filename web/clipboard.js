export async function askForPermission(name) {
   // On browsers using Blink, permissions are implemented and we should ask for them first,
   // and if the browser has allowed us, we can use the clipboard.
   //
   // There are permissions on Gecko, but no needed permissions for the clipboard.
   // On WebKit, permissions are not implemented.
   // You can force clipboard with the _FORCE_CLIPBOARD entry in local storage. See above.
   if (navigator.permissions) {
      const permission = await navigator.permissions.query({ name });
      return permission.state == "granted" || permission.state == "prompt";
   } else {
      return false;
   }
}

// HACK: In normal circumstances, we should read text from clipboard *on demand*,
// but because text field is synchronous, while readText is not, it is really hard to do it
// properly. In theory we could make text field asynchronous too, but async fn in trait is
// still unstable, and doesn't support dyn traits.
// I tried with callbacks, but rustc was mad about lifetimes, so it didn't work.
// TODO: Find better solution (unless it is the best possible solution)
let clipboardContent = "";

export function init() {
   if (navigator.clipboard.readText) {
      setInterval(() => {
         navigator.clipboard.readText()
            .then((text) => {
               clipboardContent = text;
            })
            .catch(() => {
               clipboardContent = "";
            });
      }, 1000 / 60);
   }
}

export function copyString(string) {
   navigator.clipboard.writeText(string);
}

export function copyImage(image) {
   const type = "image/png";
   const blob = new Blob([image], { type });
   const data = [
      new ClipboardItem({ [type]: blob })
   ];
   navigator.clipboard.write(data);
}

export function pasteString() {
   return clipboardContent;
}

export async function pasteImage() {
   const contents = await navigator.clipboard.read();
   for (const item of contents) {
      if (!item.types.includes('image/png')) {
         throw new Error('Clipboard contains non-image data.');
      }
      const blob = await item.getType('image/png');
      const buffer = new Uint8Array(await blob.arrayBuffer());
      return buffer;
   }
}
