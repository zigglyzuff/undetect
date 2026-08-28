const path = require('path');

module.exports = {
  packagerConfig: {
    name: 'Undetect',
    executableName: 'Undetect',
    asar: true,
    icon: path.join(__dirname, 'src/assets/icon.ico'),
    ignore: [
      /^\/\.git/,
      /^\/out/,
      /^\/\.vscode/,
      /^\/scratch/,
      /\.md$/,
      /\.log$/,
    ],
  },
  rebuildConfig: {},
  makers: [
    {
      name: '@electron-forge/maker-squirrel',
      config: {
        name: 'undetect',
        productName: 'Undetect',
        shortcutName: 'Undetect',
        setupIcon: path.join(__dirname, 'src/assets/icon.ico'),
        createDesktopShortcut: true,
        createStartMenuShortcut: true,
      },
    },
    {
      name: '@electron-forge/maker-zip',
      platforms: ['win32'],
    },
  ],
};
