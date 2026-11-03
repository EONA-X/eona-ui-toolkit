import { parseTurtle } from './turtle.js';
import { graphToModel } from '../src/rdf.js';
import { normalize } from '../src/model.js';

/**
 * @param {string} text Turtle source
 * @param {{base?: string, menu?: string}} [opts]
 */
export function ttlToModel(text, opts = {}) {
  return normalize(graphToModel(parseTurtle(text, opts).triples, opts));
}
