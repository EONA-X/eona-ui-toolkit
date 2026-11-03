export class MenuError extends Error {
  /** @param {string} message @param {{path:string,message:string}[]} [errors] */
  constructor(message, errors = []) { super(message); this.name = 'MenuError'; this.errors = errors; }
}
