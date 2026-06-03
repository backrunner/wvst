export default {
  input: {
    effect: "dist/esm/effect/main.js",
    instrument: "dist/esm/instrument/main.js",
  },
  external: ["@wvst/web"],
  output: {
    dir: "dist",
    entryFileNames: "[name]/main.js",
    format: "es",
  },
};
