/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: ['#0D1117', '#161B22', '#21262D'],
        primary: ['#00D9FF', '#8B5CF6'],
        text: ['#FFFFFF', '#8B949E'],
        success: '#10B981',
        error: '#EF4444',
        warning: '#F59E0B',
      },
      fontFamily: {
        sans: ['Microsoft YaHei', 'PingFang-SC', 'sans-serif'],
      },
    },
  },
  plugins: [
    require('tailwindcss-animate'),
  ],
}
