import { createApp } from "vue";
import "./styles/themes.css";
import App from "./App.vue";
import { initGlobalTheme } from "./composables/useTheme";

function initApp() {
  // 初始化主题系统（登录前先用本地缓存的设置，登录后 App.vue 会再次同步）
  initGlobalTheme().then(() => {
    console.log('Theme system initialized');
  }).catch((error) => {
    console.error('Failed to initialize theme system:', error);
  });

  // 禁用浏览器默认右键菜单
  document.addEventListener('DOMContentLoaded', () => {
    document.addEventListener('contextmenu', (e) => {
      // 检查是否点击在自定义右键菜单组件上
      const target = e.target as Element;
      const isCustomMenu = target.closest('.context-menu') ||
                          target.closest('.menu-item') ||
                          target.closest('[data-custom-menu]');

      // 如果不是自定义菜单组件，则阻止默认右键菜单
      if (!isCustomMenu) {
        e.preventDefault();
      }
    });
  });

  createApp(App).mount("#app");
}

initApp();
