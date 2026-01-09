import DefaultTheme from 'vitepress/theme'
import './style.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app, router }) {
    // Make logo click go to main site
    if (typeof window !== 'undefined') {
      setTimeout(() => {
        const logo = document.querySelector('.VPNavBarTitle')
        if (logo) {
          logo.style.cursor = 'pointer'
          logo.addEventListener('click', (e) => {
            e.preventDefault()
            window.location.href = 'https://rbxsync.dev'
          })
        }
      }, 100)
    }
  }
}
