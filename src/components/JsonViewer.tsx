import JsonView from "react18-json-view";
import "react18-json-view/src/style.css";

interface JsonViewerProps {
  src: string;
}

/** 解析 JSON 字符串并渲染可折叠树，解析失败时降级为纯文本 */
export function JsonViewer({ src }: JsonViewerProps) {
  let parsed: unknown;
  try {
    parsed = JSON.parse(src);
  } catch {
    return <pre className="logDetailBody">{src}</pre>;
  }
  return (
    <div className="logJsonViewer">
      <JsonView src={parsed} collapseStringsAfterLength={200} enableClipboard={false} />
    </div>
  );
}
