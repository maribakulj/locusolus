import pg from "pg";

export const cursor = () => new pg.Client();
