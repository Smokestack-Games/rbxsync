import DefaultTheme from 'vitepress/theme'
import CustomNav from './CustomNav.vue'
import './style.css'
import { h } from 'vue'

export default {
  extends: DefaultTheme,
  Layout() {
    return h(DefaultTheme.Layout, null, {
      'nav-bar-content-after': () => h(CustomNav)
    })
  },
  enhanceApp({ app, router }) {
    if (typeof window !== 'undefined') {
      const setupLogoLink = () => {
        const logo = document.querySelector('.VPNavBarTitle')
        if (logo) {
          logo.style.cursor = 'pointer'
          logo.addEventListener('click', (e) => {
            e.preventDefault()
            e.stopPropagation()
            window.location.href = 'https://rbxsync.dev'
          })
        }
      }

      // Run on initial load
      setTimeout(setupLogoLink, 100)

      // Fetch GitHub stars
      fetch('https://api.github.com/repos/devmarissa/rbxsync')
        .then(res => res.json())
        .then(data => {
          const updateStars = () => {
            const starCount = document.querySelector('.star-count')
            if (starCount && data.stargazers_count !== undefined) {
              starCount.textContent = data.stargazers_count
            }
          }
          updateStars()
          // Also update after a delay in case component mounts late
          setTimeout(updateStars, 500)
        })
        .catch(() => {})
    }
  }
}
