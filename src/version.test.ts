import { describe,expect,it } from "vitest"; import { newer } from "./App";
describe("version comparison",()=>{it("compares numeric segments",()=>{expect(newer("v1.10.0","1.9.9")).toBe(true);expect(newer("1.2.0","1.2.0")).toBe(false);expect(newer("1.1.9","1.2.0")).toBe(false)})});
