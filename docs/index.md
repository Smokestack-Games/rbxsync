---
layout: page
# RbxSync Documentation
---

<script setup>
import { onMounted } from 'vue'
import { useRouter } from 'vitepress'

onMounted(() => {
  const router = useRouter()
  router.go('/getting-started/')
})
</script>

<div style="display: flex; align-items: center; justify-content: center; height: 50vh; color: var(--vp-c-text-2);">
  Redirecting to documentation...
</div>
