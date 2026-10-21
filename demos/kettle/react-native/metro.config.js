const path = require('path');
const { getDefaultConfig, mergeConfig } = require('@react-native/metro-config');

// The Expectation driver is a local package (clients/js), linked from node_modules.
const driver = path.resolve(__dirname, '../../../clients/js');

/** @type {import('@react-native/metro-config').MetroConfig} */
const config = {
  watchFolders: [driver],
  resolver: { nodeModulesPaths: [path.resolve(__dirname, 'node_modules')] },
};

module.exports = mergeConfig(getDefaultConfig(__dirname), config);
