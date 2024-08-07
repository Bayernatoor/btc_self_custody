/** @type {import('tailwindcss').Config} */
module.exports = {
  //important: true,
  content: { 
      files: ["*.html", "./src/**/*.rs"]
  },
  theme: {
    fontFamily: {
        questrial: ["Questrial", "sans-serif"],
        title: ["Oswald", "sans-serif"]
    },
    extend: {
      animation: {
          'fadein': 'fadein 2s ease-in-out forwards',
          'fadeinone': 'fadeinone 1s ease-in-out forwards',
          'slideout': 'slideout 1s ease-in-out',
          'slidein': 'slidein 1s ease-in-out',
          'slideinfast': 'slidein 0.25s ease-out',
          'zoominimg': 'transform 0.25 ease',
         },
      keyframes: {
          fadein: {
              '0%': { opacity: 0},
              '100%': { opacity: 1},
          },
          fadeinone: {
              '0%': { opacity: 0},
              '100%': { opacity: 1},
          },
          slideout: {
              '0%': { transform: 'translateX(0%)', opacity: 1 },
              '99.999%': {opacity: 0},
              '100%' : { transform: 'translateX(-100%)', opacity: 1 },
          },
          slidein: {
              '0%': { transform: 'translateX(100%)', opacity: 1 },
              '100%' : { transform: 'translateX(0%)', opacity: 1 },
          },
          slideinfast: {
              '0%': { transform: 'translateX(100%)', opacity: 1 },
              '100%' : { transform: 'translateX(0%)', opacity: 1 },
          },
          zoominimg: {
              '0%': { transform: 'scale(0)', opacity: 1 },
              '100%' : { transform: 'scale(2)', opacity: 1 },
          },
      },
    },
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
}
