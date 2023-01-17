const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');

module.exports = {
   entry: ['./bootstrap.js', './socket.js', './common.js', './clipboard.js'],
   output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'bootstrap.js',
   },
   resolve: {
      alias: {
         socket$: path.resolve(__dirname, 'socket.js'),
         common$: path.resolve(__dirname, 'common.js'),
         clipboard$: path.resolve(__dirname, 'clipboard.js')
      },
   },
   plugins: [
      new HtmlWebpackPlugin({
         template: './index.html',
      }),
      new WasmPackPlugin({
         crateDirectory: path.resolve(__dirname, '..')
      }),
   ],
   mode: 'development',
   experiments: {
      asyncWebAssembly: true
   }
};
