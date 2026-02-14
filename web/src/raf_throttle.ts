type RafRequest = (cb: FrameRequestCallback) => number;
type RafCancel = (id: number) => void;

type CreateRafThrottleArgs = {
  requestAnimationFrame?: RafRequest;
  cancelAnimationFrame?: RafCancel;
};

export type RafThrottleController = {
  schedule: () => void;
  cancel: () => void;
  isPending: () => boolean;
};

export function createRafThrottle(
  callback: () => void,
  args: CreateRafThrottleArgs = {}
): RafThrottleController {
  const requestFrame = args.requestAnimationFrame;
  const cancelFrame = args.cancelAnimationFrame;
  let frameId: number | null = null;

  return {
    schedule: () => {
      if (!requestFrame) {
        callback();
        return;
      }
      if (frameId != null) return;
      frameId = requestFrame(() => {
        frameId = null;
        callback();
      });
    },
    cancel: () => {
      if (frameId == null || !cancelFrame) return;
      cancelFrame(frameId);
      frameId = null;
    },
    isPending: () => frameId != null,
  };
}
