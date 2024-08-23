const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
   entry: "./src/bootstrap.js",
   output: {
      path: path.resolve(__dirname, "dist"),
      filename: "bootstrap.js",
   },
   plugins: [
      new HtmlWebpackPlugin({
         template: "./src/index.html",
      }),
      new WasmPackPlugin({
         crateDirectory: path.resolve(__dirname, ".."),
      }),
   ],
   mode: "development",
   experiments: {
      asyncWebAssembly: true,
      syncWebAssembly: true,
   },
};
