import pg from "pg";

export const connect = () => new pg.Client();
