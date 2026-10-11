// @vitest-environment jsdom
import {act, cleanup, renderHook} from "@testing-library/react";
import {afterEach, expect, it} from "vitest";
import {parseEncoderNavigation, useEncoderNavigation} from "./useEncoderNavigation";
afterEach(cleanup);
it("shares family/page between surfaces and preserves it across remounts without crossing desks", () => {
 const first = renderHook(() => useEncoderNavigation("navigation-test-owner"));
 const mirror = renderHook(() => useEncoderNavigation("navigation-test-owner"));
 const other = renderHook(() => useEncoderNavigation("navigation-test-other"));
 act(() => first.result.current[1]("Color", 2));
 expect(mirror.result.current[0]).toMatchObject({family:"Color",page:2});
 expect(other.result.current[0]).toMatchObject({family:"Intensity",page:1});
 first.unmount();
 const reopened = renderHook(() => useEncoderNavigation("navigation-test-owner"));
 expect(reopened.result.current[0]).toMatchObject({family:"Color",page:2});
 act(() => mirror.result.current[1]("Position", 3));
 expect(reopened.result.current[0]).toMatchObject({family:"Position",page:3});
});
it("adopts navigation from another window and ignores malformed storage", () => {
 const owner = renderHook(() => useEncoderNavigation("navigation-test-storage"));
 act(() => window.dispatchEvent(new StorageEvent("storage", {key:"tosklight:encoder-navigation:navigation-test-storage",newValue:JSON.stringify({family:"Beam",page:2})})));
 expect(owner.result.current[0]).toMatchObject({family:"Beam",page:2});
 act(() => window.dispatchEvent(new StorageEvent("storage", {key:"tosklight:encoder-navigation:navigation-test-storage",newValue:'{"family":"Oops","page":-1}'})));
 expect(owner.result.current[0]).toMatchObject({family:"Beam",page:2});
 expect(parseEncoderNavigation("broken")).toBeNull();
});
it("shares Dynamics and owner context, and lets a mirror select a family to leave Dynamics", () => {
 const owner = renderHook(() => useEncoderNavigation("navigation-test-context"));
 const mirror = renderHook(() => useEncoderNavigation("navigation-test-context"));
 act(() => owner.result.current[2].setDynamicsMode(true));
 expect(mirror.result.current[0].dynamicsMode).toBe(true);
 act(() => mirror.result.current[1]("Intensity", 1));
 expect(owner.result.current[0].dynamicsMode).toBe(false);
 act(() => owner.result.current[2].publishOwnerContext("stage"));
 expect(mirror.result.current[0].ownerContext).toBe("stage");
});
it("receives owner context and Dynamics from another window", () => {
 const mirror = renderHook(() => useEncoderNavigation("navigation-test-cross-context"));
 act(() => {
  window.dispatchEvent(new StorageEvent("storage", {key:"tosklight:encoder-navigation:navigation-test-cross-context",newValue:JSON.stringify({family:"Color",page:2,dynamicsMode:true})}));
  window.dispatchEvent(new StorageEvent("storage", {key:"tosklight:encoder-navigation:navigation-test-cross-context:owner-context",newValue:JSON.stringify("dynamic-editor")}));
 });
 expect(mirror.result.current[0]).toEqual({family:"Color",page:2,dynamicsMode:true,ownerContext:"dynamic-editor"});
});
it("does not let a stale mirror navigation event replace newer owner context", () => {
 const owner = renderHook(() => useEncoderNavigation("navigation-test-race"));
 act(() => owner.result.current[2].publishOwnerContext("stage"));
 act(() => window.dispatchEvent(new StorageEvent("storage", {key:"tosklight:encoder-navigation:navigation-test-race",newValue:JSON.stringify({family:"Color",page:2,dynamicsMode:false,ownerContext:"programmer"})})));
 expect(owner.result.current[0]).toMatchObject({family:"Color",page:2,ownerContext:"stage"});
});
