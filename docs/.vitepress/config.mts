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
                        {text: "System Vision", link: '/architecture/system-vision'},
                        {text: "Boot Process", link: '/architecture/boot-process'},
                        {text: "Storage and Loading", link: '/architecture/storage-and-loading'},
                        {text: "Memory Management", link: '/architecture/memory-management'},
                        {text: "Scheduler and Process Services", link: '/architecture/scheduler-and-process-services'},
                        {text: "Filesystem Layout", link: '/architecture/filesystem-layout'},
                        {text: "User and Kernel Interface", link: '/architecture/user-kernel-interface'},
                        {text: "Runtime Model", link: '/architecture/runtime-model'},
                        {text: "Text Runtime", link: '/architecture/text-runtime'},
                    ]
                }
            ]
        },

        socialLinks: [
            {icon: 'github', link: 'https://github.com/floffah/lazers'}
        ]
    }
})
