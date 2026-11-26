/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        surface: {
          DEFAULT: "#1a1a18",
          raised: "#232320",
          overlay: "#2c2c28",
        },
        edge: "#3a3a34",
        ink: {
          DEFAULT: "#e8e6e0",
          dim: "#a3a099",
          faint: "#6b6963",
        },
        accent: {
          DEFAULT: "#d97757",
          hover: "#e8896a",
        },
      },
      fontFamily: {
        sans: ["system-ui", "-apple-system", "Segoe UI", "sans-serif"],
        mono: ["ui-monospace", "SF Mono", "Menlo", "monospace"],
      },
    },
  },
  plugins: [],
};
