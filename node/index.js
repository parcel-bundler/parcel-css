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

// Include a small JS-shim for `bundleAsync()` to convert make `resolver` more ergonomic.
const {bundleAsync} = module.exports;
module.exports.bundleAsync = (opts, ...rest) => {
  return bundleAsync({
    ...opts,
    resolver: opts.resolver && {
      ...opts.resolver,
      read: opts.resolver.read && normalizeJsCallback(opts.resolver.read.bind(opts.resolver)),
      resolve: opts.resolver.resolve && normalizeJsCallback(opts.resolver.resolve.bind(opts.resolver)),
    },
  }, ...rest);
};

// `napi-rs` ignores JS function return values, so any results must be passed back to
// the Rust side via a callback rather than a returned value or `Promise`. This
// callback also follows NodeJS conventions (`callback(err, result)`). Managing the
// error and callback are annoying for users, so this converts a typical JS function
// which returns its result in a `Promise` into a N-API-compatible function which
// accepts and propagates its results to a callback.
function normalizeJsCallback(func) {
  return (...args) => {
    // Splice out `[...args, callback]`.
    const funcArgs = args.slice(0, -1);
    const callback = args[args.length - 1];

    // Invoke the inner function, normalize to a `Promise`, and then invoke the callback
    // function with Node conventions of `callback(err, result)`.
    toPromise(() => func(...funcArgs)).then(
      (result) => callback(null, result),
      (err) => callback(err, null),
    );
  };
}

// Converts the given function execution to return a `Promise` instead of returning or
// erroring synchronously. This is different from `Promise.resolve(func())` in that a
// synchronous `throw` statement is converted to a `Promise` rejection.
function toPromise(func) {
  try {
    return Promise.resolve(func());
  } catch (err) {
    return Promise.reject(err);
  }
}
