const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const ForkTsCheckerNotifierWebpackPlugin = require("fork-ts-checker-notifier-webpack-plugin");
const ForkTsCheckerWebpackPlugin = require("fork-ts-checker-webpack-plugin");

module.exports = {
   entry: "./src/bootstrap.ts",
   devtool: "inline-source-map",
   output: {
      path: path.resolve(__dirname, "dist"),
      filename: "bundle.js",
   },
   resolve: {
      alias: {
         socket$: path.resolve(__dirname, "./src/socket.ts"),
         common$: path.resolve(__dirname, "./src/common.ts"),
         clipboard$: path.resolve(__dirname, "./src/clipboard.ts"),
      },
      extensions: [".ts", ".js"],
      extensionAlias: {
         ".js": [".js", ".ts"],
         ".cjs": [".cjs", ".cts"],
         ".mjs": [".mjs", ".mts"],
      },
   },
   plugins: [
      new HtmlWebpackPlugin({
         template: "./src/index.html",
      }),
      new WasmPackPlugin({
         crateDirectory: path.resolve(__dirname, ".."),
      }),
      new ForkTsCheckerWebpackPlugin(),
      new ForkTsCheckerNotifierWebpackPlugin({
         title: "TypeScript",
         excludeWarnings: false,
      }),
   ],
   module: {
      rules: [
         {
            test: /\.([cm]?ts|tsx)$/,
            loader: "ts-loader",
            include: path.resolve(__dirname, "./src"),
         },
      ],
   },
   mode: "development",
   experiments: {
      asyncWebAssembly: true,
      syncWebAssembly: true,
   },
};
