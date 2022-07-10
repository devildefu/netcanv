const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = {
   entry: ['./bootstrap.js', './socket.js'],
   output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'bootstrap.js',
   },
   resolve: {
      alias: {
         socket$: path.resolve(__dirname, 'socket.js')
      },
   },
   plugins: [
      new HtmlWebpackPlugin({
         template: './index.html',
     }),
   ],
   mode: 'development',
   experiments: {
      asyncWebAssembly: true
   }
};