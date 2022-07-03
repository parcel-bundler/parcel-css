import type { Targets } from './targets';

export interface TransformOptions {
  /** The filename being transformed. Used for error messages and source maps. */
  filename: string,
  /** The source code to transform. */
  code: Buffer,
  /** Whether to enable minification. */
  minify?: boolean,
  /** Whether to output a source map. */
  sourceMap?: boolean,
  /** The browser targets for the generated code. */
  targets?: Targets,
  /** Whether to enable various draft syntax. */
  drafts?: Drafts,
  /** Whether to compile this file as a CSS module. */
  cssModules?: boolean | CSSModulesConfig,
  /**
   * Whether to analyze dependencies (e.g. `@import` and `url()`).
   * When enabled, `@import` rules are removed, and `url()` dependencies
   * are replaced with hashed placeholders that can be replaced with the final
   * urls later (after bundling). Dependencies are returned as part of the result.
   */
  analyzeDependencies?: boolean,
  /** 
   * Replaces user action pseudo classes with class names that can be applied from JavaScript.
   * This is useful for polyfills, for example.
   */
  pseudoClasses?: PseudoClasses,
  /**
   * A list of class names, ids, and custom identifiers (e.g. @keyframes) that are known
   * to be unused. These will be removed during minification. Note that these are not
   * selectors but individual names (without any . or # prefixes).
   */
  unusedSymbols?: string[]
}

export type BundleOptions = Omit<TransformOptions, 'code'>;

export interface Drafts {
  /** Whether to enable CSS nesting. */
  nesting?: boolean,
  /** Whether to enable @custom-media rules. */
  customMedia?: boolean
}

export interface PseudoClasses {
  hover?: string,
  active?: string,
  focus?: string,
  focusVisible?: string,
  focusWithin?: string
}

export interface TransformResult {
  /** The transformed code. */
  code: Buffer,
  /** The generated source map, if enabled. */
  map: Buffer | void,
  /** CSS module exports, if enabled. */
  exports: CSSModuleExports | void,
  /** CSS module references, if `dashedIdents` is enabled. */
  references: CSSModuleReferences,
  /** `@import` and `url()` dependencies, if enabled. */
  dependencies: Dependency[] | void
}

export interface CSSModulesConfig {
  /** The pattern to use when renaming class names and other identifiers. Default is `[hash]_[local]`. */
  pattern: string,
  /** Whether to rename dashed identifiers, e.g. custom properties. */
  dashedIdents: boolean
}

export type CSSModuleExports = {
  /** Maps exported (i.e. original) names to local names. */
  [name: string]: CSSModuleExport
};

export interface CSSModuleExport {
  /** The local (compiled) name for this export. */
  name: string,
  /** Whether the export is referenced in this file. */
  isReferenced: boolean,
  /** Other names that are composed by this export. */
  composes: CSSModuleReference[]
}

export type CSSModuleReferences = {
  /** Maps placeholder names to references. */
  [name: string]: DependencyCSSModuleReference,
};

export type CSSModuleReference = LocalCSSModuleReference | GlobalCSSModuleReference | DependencyCSSModuleReference;

export interface LocalCSSModuleReference {
  type: 'local',
  /** The local (compiled) name for the reference. */
  name: string,
}

export interface GlobalCSSModuleReference {
  type: 'global',
  /** The referenced global name. */
  name: string,
}

export interface DependencyCSSModuleReference {
  type: 'dependency',
  /** The name to reference within the dependency. */
  name: string,
  /** The dependency specifier for the referenced file. */
  specifier: string
}

export type Dependency = ImportDependency | UrlDependency;

export interface ImportDependency {
  type: 'import',
  /** The url of the `@import` dependency. */
  url: string,
  /** The media query for the `@import` rule. */
  media: string | null,
  /** The `supports()` query for the `@import` rule. */
  supports: string | null,
  /** The source location where the `@import` rule was found. */
  loc: SourceLocation
}

export interface UrlDependency {
  type: 'url',
  /** The url of the dependency. */
  url: string,
  /** The source location where the `url()` was found. */
  loc: SourceLocation,
  /** The placeholder that the url was replaced with. */
  placeholder: string
}

export interface SourceLocation {
  /** The file path in which the dependency exists. */
  filePath: string,
  /** The start location of the dependency. */
  start: Location,
  /** The end location (inclusive) of the dependency. */
  end: Location
}

export interface Location {
  /** The line number (1-based). */
  line: number,
  /** The column number (0-based). */
  column: number
}

/**
 * Compiles a CSS file, including optionally minifying and lowering syntax to the given
 * targets. A source map may also be generated, but this is not enabled by default.
 */
export declare function transform(options: TransformOptions): TransformResult;

export interface TransformAttributeOptions {
  /** The source code to transform. */
  code: Buffer,
  /** Whether to enable minification. */
  minify?: boolean,
  /** The browser targets for the generated code. */
  targets?: Targets,
  /**
   * Whether to analyze `url()` dependencies.
   * When enabled, `url()` dependencies are replaced with hashed placeholders 
   * that can be replaced with the final urls later (after bundling).
   * Dependencies are returned as part of the result.
   */
  analyzeDependencies?: boolean
}

export interface TransformAttributeResult {
  /** The transformed code. */
  code: Buffer,
  /** `@import` and `url()` dependencies, if enabled. */
  dependencies: Dependency[] | void
}

/**
 * Compiles a single CSS declaration list, such as an inline style attribute in HTML.
 */
export declare function transformStyleAttribute(options: TransformAttributeOptions): TransformAttributeResult;

/**
 * Converts a browserslist result into targets that can be passed to @parcel/css.
 * @param browserslist the result of calling `browserslist`
 */
export declare function browserslistToTargets(browserslist: string[]): Targets;

/**
 * Bundles a CSS file and its dependencies, inlining @import rules.
 */
export declare function bundle(options: BundleOptions): TransformResult;

export interface AsyncBundleOptions extends BundleOptions {
  resolver?: Resolver;
}

/** Custom resolver to use when loading CSS files. */
export interface Resolver {
  /** Read the given file and return its contents as a string. */
  read?: (file: string) => string | Promise<string>;

  /**
   * Resolve the given CSS import specifier from the provided originating file to a
   * path which gets passed to `read()`.
   */
  resolve?: (specifier: string, originatingFile: string) => string | Promise<string>;
}

/**
 * Bundles a CSS file and its dependencies asynchronously, inlining @import rules.
 * Also supports custom resolvers.
 */
export declare function bundleAsync(options: AsyncBundleOptions): Promise<TransformResult>;
