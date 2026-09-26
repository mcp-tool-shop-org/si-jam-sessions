// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';

// https://astro.build/config
export default defineConfig({
  site: 'https://mcp-tool-shop-org.github.io',
  base: '/si-jam-sessions',
  integrations: [
    starlight({
      title: 'si-jam-sessions',
      description:
        "A music engine where an AI's playing can be graded exactly, replayed exactly, and trained on with a clean conscience.",
      logo: { src: './src/assets/logo.png', alt: 'si-jam-sessions', href: '/si-jam-sessions/', replacesTitle: false },
      disable404Route: true,
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/mcp-tool-shop-org/si-jam-sessions' },
      ],
      sidebar: [
        {
          label: 'Handbook',
          items: [{ autogenerate: { directory: 'handbook' } }],
        },
      ],
      customCss: ['./src/styles/starlight-custom.css'],
    }),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
