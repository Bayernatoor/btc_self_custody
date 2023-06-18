/** @type {import('tailwindcss').Config} */
module.exports = {
  //important: true,
  content: { 
      files: ["*.html", "./src/**/*.rs"]
  },
  theme: {
    extend: {
     animation: {
         'fadein': 'fadein 2s ease-in-out forwards',
        },
    keyframes: {
        fadein: {
            '0%': { opacity: 0},
            '100%': { opacity: 1},
        },
    },
  },
},
  plugins: [],
}

