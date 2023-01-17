export function showSaveFilePicker(buffer) {
   const url = URL.createObjectURL(
      new Blob([buffer], { type: "image/png"})
   );

   const anchor = document.createElement("a");
   anchor.setAttribute("href", url);
   anchor.setAttribute("download", "canvas.png");
   anchor.click();
}
