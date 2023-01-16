const rust = import('../pkg');

rust
  .then(wasm => {
    wasm.start();
  })
  .catch(console.error);