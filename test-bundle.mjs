import path from 'path';
import css from './node/index.js';

(function testBundleRejectsResolver() {
    let error = undefined;
    try {
        css.bundle({
            filename: 'foo.css',
            resolver: {},
        });
    } catch (err) {
        error = err;
    }

    if (!error) throw new Error(`\`testBundleRejectsResolver()\` failed. Expected \`bundle()\` to throw, but it did not.`);
    if (!error.message.includes(`\`bundle()\` doesn't support custom JS resolvers`) || !error.message.includes(`Use \`bundleAsync()\` instead.`)) {
        throw new Error(`\`testBundleRejectsResolver()\` failed. Expected \`bundle()\` to throw a specific error message, but it threw a different error:\n${error.message}`);
    }
}());

(async function testResolver() {
    const inMemoryFs = new Map(Object.entries({
        'foo.css': `
@import 'root:bar.css';

.foo { color: red; }
        `.trim(),

        'bar.css': `
@import 'root:hello/world.css';

.bar { color: green; }
        `.trim(),

        'hello/world.css': `
.baz { color: blue; }
        `.trim(),
    }));

    const { code: buffer } = await css.bundleAsync({
        filename: 'foo.css',
        resolver: {
            read(file) {
                const result = inMemoryFs.get(path.normalize(file));
                if (!result) throw new Error(`Could not find ${file} in ${
                    Array.from(inMemoryFs.keys()).join(', ')}.`);
                return result;
            },

            resolve(specifier) {
                return specifier.slice('root:'.length);
            },
        },
    });
    const code = buffer.toString('utf-8').trim();

    const expected = `
.baz {
  color: #00f;
}

.bar {
  color: green;
}

.foo {
  color: red;
}
    `.trim();
    if (code !== expected) throw new Error(`\`testResolver()\` failed. Expected:\n${expected}\n\nGot:\n${code}`);
})();

(async function testOnlyCustomRead() {
    const inMemoryFs = new Map(Object.entries({
        'foo.css': `
@import 'hello/world.css';

.foo { color: red; }
        `.trim(),

        'hello/world.css': `
@import '../bar.css';

.bar { color: green; }
        `.trim(),

        'bar.css': `
.baz { color: blue; }
        `.trim(),
    }));

    const { code: buffer } = await css.bundleAsync({
        filename: 'foo.css',
        resolver: {
            read(file) {
                const result = inMemoryFs.get(path.normalize(file));
                if (!result) throw new Error(`Could not find ${file} in ${
                    Array.from(inMemoryFs.keys()).join(', ')}.`);
                return result;
            },
        },
    });
    const code = buffer.toString('utf-8').trim();

    const expected = `
.baz {
  color: #00f;
}

.bar {
  color: green;
}

.foo {
  color: red;
}
    `.trim();
    if (code !== expected) throw new Error(`\`testOnlyCustomRead()\` failed. Expected:\n${expected}\n\nGot:\n${code}`);
})();

(async function testOnlyCustomResolve() {
    const root = path.join('tests', 'testdata', 'css');
    const { code: buffer } = await css.bundleAsync({
        filename: path.join(root, 'foo.css'),
        resolver: {
            resolve(specifier) {
                // Strip `root:` prefix off specifier and resolve it as an absolute path
                // in the test data root.
                return path.join(root, specifier.slice('root:'.length));
            },
        },
    });
    const code = buffer.toString('utf-8').trim();

    const expected = `
.baz {
  color: #00f;
}

.bar {
  color: green;
}

.foo {
  color: red;
}
    `.trim();
    if (code !== expected) throw new Error(`\`testOnlyCustomResolve()\` failed. Expected:\n${expected}\n\nGot:\n${code}`);
})();

(async function testReadThrow() {
    let error = undefined;
    try {
        await css.bundleAsync({
            filename: 'foo.css',
            resolver: {
                read(file) {
                    throw new Error(`Oh noes! Failed to read \`${file}\`.`);
                }
            },
        });
    } catch (err) {
        error = err;
    }

    if (!error) throw new Error(`\`testReadThrow()\` failed. Expected \`bundleAsync()\` to throw, but it did not.`);
    if (!error.message.includes(`\`read()\` threw error:`) || !error.message.includes(`Oh noes! Failed to read \`foo.css\`.`)) {
        throw new Error(`\`testReadThrow()\` failed. Expected \`bundleAsync()\` to throw a specific error message, but it threw a different error:\n${error.message}`);
    }
})();

(async function testResolveThrow() {
    let error = undefined;
    try {
        await css.bundleAsync({
            filename: 'tests/testdata/css/foo.css',
            resolver: {
                resolve(specifier, originatingFile) {
                    throw new Error(`Oh noes! Failed to resolve \`${specifier}\` from \`${
                        originatingFile}\`.`);
                }
            },
        });
    } catch (err) {
        error = err;
    }

    if (!error) throw new Error(`\`testResolveThrow()\` failed. Expected \`bundleAsync()\` to throw, but it did not.`);
    if (!error.message.includes(`\`resolve()\` threw error:`) || !error.message.includes(`Oh noes! Failed to resolve \`root:hello/world.css\` from \`tests/testdata/css/foo.css\`.`)) {
        throw new Error(`\`testResolveThrow()\` failed. Expected \`bundleAsync()\` to throw a specific error message, but it threw a different error:\n${error.message}`);
    }
})();

(async function testReadReturnNonString() {
    let error = undefined;
    try {
        await css.bundleAsync({
            filename: 'foo.css',
            resolver: {
                read() {
                    return 1234; // Returns a non-string value.
                }
            },
        });
    } catch (err) {
        error = err;
    }

    if (!error) throw new Error(`\`testReadReturnNonString()\` failed. Expected \`bundleAsync()\` to throw, but it did not.`);
    if (!error.message.includes(`Expected \`read()\` to return a value of type \`String\`, but it returned a value of type \`Number\` instead.`)) {
        throw new Error(`\`testReadReturnNonString()\` failed. Expected \`bundleAsync()\` to throw a specific error message, but it threw a different error:\n${error.message}`);
    }
})();

(async function testResolveReturnNonString() {
    let error = undefined;
    try {
        await css.bundleAsync({
            filename: 'tests/testdata/css/foo.css',
            resolver: {
                resolve() {
                    return 1234; // Returns a non-string value.
                }
            },
        });
    } catch (err) {
        error = err;
    }

    if (!error) throw new Error(`\`testResolveReturnNonString()\` failed. Expected \`bundleAsync()\` to throw, but it did not.`);
    if (!error.message.includes(`Expected \`resolve()\` to return a value of type \`String\`, but it returned a value of type \`Number\` instead.`)) {
        throw new Error(`\`testResolveReturnNonString()\` failed. Expected \`bundleAsync()\` to throw a specific error message, but it threw a different error:\n${error.message}`);
    }
})();

(async function testThis() {
    const inMemoryFs = new Map(Object.entries({
        'foo.css': `
@import './bar.css';

.foo { color: red; }
        `.trim(),

        'bar.css': `
@import './hello/world.css';

.bar { color: green; }
        `.trim(),

        'hello/world.css': `
.baz { color: blue; }
        `.trim(),
    }));

    let readThis = undefined;
    let resolveThis = undefined;
    const resolver = {
        read: function (file) {
            readThis = this;
            const result = inMemoryFs.get(path.normalize(file));
            if (!result) throw new Error(`Could not find ${file} in ${
                Array.from(inMemoryFs.keys()).join(', ')}.`);
            return result;
        },
        resolve: function (specifier, originatingFile) {
            resolveThis = this;
            return path.join(originatingFile, '..', specifier);
        },
    };
    await css.bundleAsync({
        filename: 'foo.css',
        resolver,
    });

    if (readThis !== resolver) throw new Error(`\`testThis()\` failed. Expected \`read()\` to be called with the \`resolver\` as \`this\`, but instead it was called with:\n${readThis}`);
    if (resolveThis !== resolver) throw new Error(`\`testThis()\` failed. Expected \`resolve()\` to be called with the \`resolver\` as \`this\`, but instead it was called with:\n${resolveThis}`);
})();

console.log('PASSED!');
