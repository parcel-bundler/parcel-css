let parts = [process.platform, process.arch];
if (process.platform === 'linux') {
  const {MUSL, family} = require('detect-libc');
  if (family === MUSL) {
    parts.push('musl');
  } else if (process.arch === 'arm') {
    parts.push('gnueabihf');
  } else {
    parts.push('gnu');
  }
} else if (process.platform === 'win32') {
  parts.push('msvc');
}

if (process.env.CSS_TRANSFORMER_WASM) {
  module.exports = require(`../pkg`);
} else {
  try {
    module.exports = require(`@parcel/css-${parts.join('-')}`);
  } catch (err) {
    module.exports = require(`../parcel-css.${parts.join('-')}.node`);
  }
}

module.exports.browserslistToTargets = require('./browserslistToTargets');
