import * as wasm from "netcanv";

function typedArrayToBuffer(array: Uint8Array) {
   return array.buffer.slice(
      array.byteOffset,
      array.byteLength + array.byteOffset
   );
}

export class SocketImpl {
   private sendQueue: ArrayBuffer[];
   private recvQueue: ArrayBuffer[];
   private isConnected: boolean;
   private isVersionOk: boolean;
   private socket: WebSocket;

   public constructor(address: string) {
      this.sendQueue = [];
      this.recvQueue = [];
      this.isConnected = false;
      this.isVersionOk = false;

      this.socket = new WebSocket(address);
      this.socket.binaryType = "arraybuffer";
      this.socket.onmessage = (event) => {
         const data = new Uint8Array(event.data);

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

   public send(data: Uint8Array) {
      if (this.isConnected) {
         this.socket.send(typedArrayToBuffer(data));
      } else {
         this.sendQueue.push(typedArrayToBuffer(data));
      }
   }

   public recv(): ArrayBuffer | null {
      const data = this.recvQueue.shift();
      if (data === undefined) return null;

      return data;
   }

   public quit() {
      this.socket.close();
   }
}
