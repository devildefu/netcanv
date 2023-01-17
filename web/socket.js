import * as wasm from "netcanv";

function typedArrayToBuffer(array) {
   return array.buffer.slice(array.byteOffset, array.byteLength + array.byteOffset)
}

export class SocketImpl {
   constructor(address) {
      this.sendQueue = [];
      this.recvQueue = [];
      this.isConnected = false;
      this.isVersionOk = false;

      this.socket = new WebSocket(address);
      this.socket.binaryType = "arraybuffer";
      this.socket.onmessage = (event) => {
         let data = new Uint8Array(event.data);

         if (this.isVersionOk === false) {
            if (wasm.checkVersion(data)) {
               console.log("version ok");
               this.isVersionOk = true;
               return;
            } else {
               this.quit();
            }
         }

         this.recvQueue.push(data);
      };
      this.socket.onclose = () => {
         console.log("Connection closed");
      };
      this.socket.onopen = () => {
         this.isConnected = true;

         for (const data of this.sendQueue) {
            this.socket.send(data);
         }
      };
   }

   send(data) {
      if (this.isConnected) {
         this.socket.send(typedArrayToBuffer(data));
      } else {
         this.sendQueue.push(typedArrayToBuffer(data));
      }
   }

   recv() {
      let data = this.recvQueue.shift();
      if (data === undefined)
         return undefined;

      return data;
   }

   quit() {
      this.socket.close();
   }
}
