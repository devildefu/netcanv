const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = {
    entry: './bootstrap.js',
    output: {
        path: path.resolve(__dirname, 'dist'),
        filename: 'bootstrap.js',
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