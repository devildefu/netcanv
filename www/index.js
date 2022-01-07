let sockets = {};

let receive;

class SocketWrapper {
  /**
   * @param {number} token
   * @param {string} address
   */
  constructor(token, address) {
    this.queue = [];
    this.isConnected = false;

    this.socket = new WebSocket(address);

    console.log(`Connecting to ${address}`);

    this.socket.binaryType = "arraybuffer";

    this.socket.onmessage = (event) => {
      receive(event, token);
    };

    this.socket.onclose = () => {
      console.log('Connection closed.');
    };

    this.socket.onopen = () => {
      this.isConnected = true;
      console.log('Connected');

      this.queue.forEach((data) => {
        this.socket.send(data);
      });
    };
  }

  /**
   * @param {ArrayBuffer} data
   */
  send(data) {
    if (this.isConnected) {
      this.socket.send(data);
    } else {
      this.queue.push(data);
    }
  }
}

export function connect(token, address) {
  let socket = new SocketWrapper(token, address);

  sockets[token] = socket;
}

export function send(data, token) {
  sockets[token].send(data.buffer);
}

export function createClipboardItem(mime, blob) {
  return [new ClipboardItem({ [mime]: blob })];
}

const rust = import('../pkg');

rust
  .then(wasm => {
    receive = wasm.receive;
    wasm.start();
  })
  .catch(console.error);