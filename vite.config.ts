import { defineConfig } from "vite";

// 纯桌面端：开发服务器只绑定本机回环地址，不对外提供网页访问。
export default defineConfig({
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
  },
});
