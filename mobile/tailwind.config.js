/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./app/**/*.{ts,tsx}", "./src/**/*.{ts,tsx}"],
  presets: [require("nativewind/preset")],
  theme: {
    extend: {
      colors: {
        primary: "#6366f1",
        "primary-dark": "#4f46e5",
        wellness: {
          bg: "#0a0a0f",
          surface: "#111118",
          accent: "#4ecdc4",
          bill: "#ff6b6b",
          important: "#ffd93d",
          muted: "#666666",
        },
      },
    },
  },
  plugins: [],
};
