import {defineConfig} from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
    title: "Lazers Docs",
    description: "Hobbyist operating system",
    themeConfig: {
        // https://vitepress.dev/reference/default-theme-config
        nav: [
            {text: 'Get Started', link: '/usage'},
            {text: "Architecture", link: '/architecture'},
        ],

        sidebar: {
            "/usage": [
                {
                    text: 'Getting Started',
                    link: "/usage",
                }
            ],
            "/architecture": [
                {
                    text: 'Architecture',
                    items: [
                        {text: 'Boot Process', link: '/architecture/boot-process'},
                        {text: 'Kernel', link: '/architecture/kernel'},
                        {text: 'User Space', link: '/architecture/user-space'},
                    ]
                }
            ]
        },

        socialLinks: [
            {icon: 'github', link: 'https://github.com/floffah/lazers'}
        ]
    }
})
