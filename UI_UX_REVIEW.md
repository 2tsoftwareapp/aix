# Báo cáo UI/UX aix

**Góc nhìn người lần đầu sử dụng aix, đã quen Codex, Claude Code và Spec Kit.**

Tôi sẽ giữ bố cục ba cột. Việc chọn bước, giao việc cho agent và đọc `spec.md`, `plan.md`, `tasks.md` trên cùng màn hình là lý do rõ nhất để dùng aix. Tuy nhiên, app hiện bắt tôi tự suy ra feature đang ở giai đoạn nào, agent sắp tác động vào đâu và kết quả nào còn cần duyệt. Đó là những việc tôi kỳ vọng một app dành riêng cho Spec Kit giúp tôi làm nhanh hơn.

| Hạng mục | Đánh giá |
|---|---|
| Bố cục nền | **7/10.** Phân vai ba cột rõ, có chỗ đối chiếu chat với tài liệu. |
| Trải nghiệm lần đầu | **4/10.** Hộp thoại API key xuất hiện trước khi người dùng thấy repo và tài liệu của mình. |
| Theo dõi và duyệt công việc | **5/10.** Có streaming, duyệt tool và trình xem tài liệu. Khả năng theo dõi lần chạy, bảo toàn bản nháp và tìm kết quả còn hạn chế. |

Các điểm trên là nhận định UX dựa trên màn hình mở đầu đã quan sát và luồng mã nguồn đã đối chiếu.

## 1. Lần đầu mở app

Ở 1440 × 900, thứ nổi bật nhất là hộp thoại `Provider settings`. Bốn provider có bốn khối gần như cùng trọng lượng thị giác. Tiêu đề và nút đóng sát mép, dòng mô tả đi ngay vào nơi lưu key, còn `Open project…` nằm phía sau lớp phủ. Với người vừa chuyển từ Codex hoặc Claude Code, câu hỏi đầu tiên là *“Tôi dùng lại phiên đăng nhập trên máy hay phải lấy API key riêng?”* Luồng desktop hiện mô tả API key và biến môi trường. Hộp thoại chưa giải thích mối quan hệ giữa cách kết nối đó và phiên đăng nhập CLI.

Tôi đã bấm `Done` để bỏ qua hộp thoại và mở được trình chọn thư mục. Nhãn `Done` chưa nói rõ tôi có thể xem repo trước khi cấu hình provider. Sau khi đóng hộp thoại, composer vẫn gộp hai điều kiện vào dòng `Open a project and configure a provider`. Người mới phải tự tìm nút mở repo ở đầu sidebar và nút cài provider ở chân sidebar.

Tôi sẽ đặt **`Open project…`** làm hành động chính trên màn hình trống. Khi đã mở repo, hiển thị feature và tài liệu nhận diện được. Khi người dùng chuẩn bị chạy bước đầu tiên, đưa `Connect provider` đến cạnh nút chạy và giải thích cách xác thực tại đó. [App](apps/desktop/src/App.tsx#L49) · [Provider settings](apps/desktop/src/components/ProviderSetup.tsx#L107) · [Trạng thái trống của chat](apps/desktop/src/components/ChatPanel.tsx#L141)

## 2. Bố cục và điều hướng

| Vùng | Cảm nhận khi dùng | Thay đổi đề xuất |
|---|---|---|
| Sidebar | Thứ tự `Project → Features → Pipeline` hợp với cách định vị task. Tên các bước quen thuộc và phần đếm task dễ lướt. Chín hàng có độ nổi tương đương, kèm số thứ tự, icon và trạng thái. Danh sách feature dài cũng có thể đẩy pipeline xuống dưới. | Làm nổi feature hiện tại và một bước đề xuất. Khi có nhiều feature, cho tìm và chuyển feature qua bộ chọn. |
| Cột giữa | Header bước và composer ở đáy giống cách làm việc với agent. Dòng hướng dẫn `Specify` giải thích đúng nội dung người dùng cần đưa vào. Khi chưa có project hoặc provider, phần trống ở giữa hướng người dùng đi tìm nút ở sidebar. | Đưa hành động giải quyết trở ngại hiện tại vào ngay vùng trống. Sau khi sẵn sàng, cho ví dụ đầu vào phù hợp với bước. |
| Cột phải | Đây là phần đáng dùng nhất. Có Markdown, sửa, lưu, sao chép và resize. Có thể đọc kết quả mà vẫn thấy cuộc trao đổi. Thanh tab ngắn nên khó biết tài liệu đang xem thuộc feature nào khi đổi ngữ cảnh. | Hiện tên file, feature và trạng thái lưu trong header của tài liệu. Cho thu gọn khi chưa có repo hoặc khi cần ưu tiên chat. |
| Composer | Provider, model và quyền tool luôn ở gần nút chạy. Ba bộ chọn rộng tổng cộng 414 px theo cấu hình, chưa tính khe hở và nút chạy. Chúng chiếm sự chú ý cả lúc ô nhập bị vô hiệu hóa. | Gom provider và model, giữ quyền tool dễ thấy, dùng nhãn nút theo hành động thực tế. |

Ở cửa sổ mặc định 1440 px, ba vùng cùng hiện rõ. Tauri đặt chiều rộng tối thiểu 960 px. Nếu sidebar giữ khoảng 280 px như ở màn hình đã xem và panel tài liệu giữ mặc định 430 px, cột giữa chỉ còn khoảng 250 px trước khi tính phần ngăn và padding. Đây là ước lượng từ bố cục. Chưa đo thao tác và hiện tượng tràn ở kích thước tối thiểu. [App shell](apps/desktop/src/App.tsx#L24) · [Composer](apps/desktop/src/components/ChatPanel.tsx#L447) · [Cấu hình cửa sổ Tauri](apps/desktop/src-tauri/tauri.conf.json)

## 3. Làm việc với repo Spec Kit có sẵn

Giả sử tôi mở repo chứa `specs/001-export-csv/spec.md` và chưa có `plan.md`. Tôi muốn nhìn thấy **`001-export-csv · Có spec · Chưa có plan`** cùng nút `Run Plan` và lựa chọn `Clarify` nếu vẫn còn câu hỏi. Pipeline hiện cho tôi thấy tên các bước, nhưng để quyết định chạy bước nào tôi còn phải diễn giải nhiều trạng thái.

| Tình huống | Hành vi và tác động | Thay đổi đề xuất |
|---|---|---|
| Mở feature đã có spec | Cùng một điều kiện có `spec.md` làm cả `Specify` và `Clarify` hiện hoàn tất. Người dùng dễ tưởng spec đã được làm rõ. | Hiện `Spec exists`. Xác nhận `Clarify` khi có bằng chứng riêng về lần làm rõ. |
| Chọn `Specify` khi đang xem feature cũ | Header có thể giữ slug cũ, panel hiển thị spec đang có, nút ghi `Run`. Backend tạo thư mục feature mới cho mỗi lần `Specify`. | Đổi nhãn thành `Create feature spec`. Cho biết đích là feature mới và đặt `Edit` hoặc `Clarify` cạnh spec hiện tại. |
| Vừa tạo feature mới | Quét lại sau `Specify` vẫn có thể chọn slug đã lưu nếu slug đó còn hợp lệ. Người dùng có thể tiếp tục nhìn vào feature cũ sau khi thấy lần chạy hoàn tất. | Chọn feature và mở file vừa tạo từ kết quả của lần chạy. |
| Chọn `Implement` khi thiếu task | Backend chặn nếu thiếu `tasks.md` hoặc chưa có mục task. Sidebar gắn ổ khóa cố định cho `Clarify`, `Checklist` và `Analyze`, còn lý do chặn thực tế chỉ hiện khi chọn bước. | Dùng trạng thái thật của feature. Ví dụ `Implement · Missing tasks.md` với lối sang `Tasks`. |
| Còn checklist chưa duyệt | Backend cho tiếp tục khi có xác nhận. Nút `Confirm & run` cho biết có cảnh báo, nhưng người dùng vẫn phải tìm checklist tương ứng. | Hiện số mục chưa duyệt, tên checklist và lối mở từng file ngay tại cảnh báo. |

**Có file, đã chạy và đã duyệt là ba trạng thái riêng.** Sidebar nên dùng chữ cạnh icon cho những trạng thái ảnh hưởng đến quyết định chạy, như `Blocked`, `Needs review` và `Ready`. [Sidebar](apps/desktop/src/components/Sidebar.tsx#L42) · [Chọn feature sau khi quét](apps/desktop/src/App.tsx#L31) · [Điều kiện chạy](apps/desktop/src-tauri/crates/speckit-core/src/gates.rs#L29) · [Lệnh chạy desktop](apps/desktop/src-tauri/src/commands.rs#L259)

## 4. Giao việc và theo dõi agent

**Trước khi chạy.** Gợi ý `/speckit.<step>` và Enter để gửi tạo cảm giác quen thuộc. Tuy nhiên, cùng một nút `Run` có thể tạo spec mới, phân tích chỉ đọc hoặc triển khai code. Trước `Implement`, tôi muốn thấy feature đích, repo đích và quyền tool ngay gần nút bấm. Có thể dùng các nhãn `Run read-only analysis` và `Implement tasks`. Mọi bước hiện đều yêu cầu prompt có chữ. Với bước như `Analyze`, khi đã có đủ tài liệu, một yêu cầu mặc định có thể giúp người dùng bắt đầu mà không phải tự nghĩ câu lệnh.

**Trong lúc chạy.** App có nội dung streaming, spinner, `Stop`, kết quả tool và thẻ `Approve`/`Deny`. Yêu cầu ghi file có thể kèm diff xem trước. Đây là các điểm nên giữ. App chưa thể hiện riêng từng trạng thái như đang gọi tool, đang chờ duyệt và đang dừng. Frontend có một cờ chạy, đưa kết quả tool vào transcript và chưa xử lý `tool_use` thành trạng thái riêng. Khi transcript dài, nên giữ trạng thái lần chạy và yêu cầu duyệt đang chờ ở vị trí dễ thấy, kèm feature của lần chạy.

**Khi chuyển ngữ cảnh hoặc gặp lỗi.** Đổi bước, feature hoặc project sẽ xóa tin nhắn và thẻ duyệt khỏi màn hình. Ví dụ tôi có thể mất phần giải thích vừa đọc ở `Clarify` khi bấm sang `Plan` rồi quay lại. Composer cũng xóa ô nhập trước khi gọi backend. Prompt đã gửi vẫn nằm trong transcript lúc giao diện báo lỗi, nhưng thiếu thao tác `Retry` hoặc `Restore prompt` ngay tại đó. Nên giữ transcript và yêu cầu duyệt theo lần chạy, giữ nháp theo feature và bước, rồi cho mở file đầu ra từ tóm tắt lần chạy. [Chat và theo dõi trạng thái](apps/desktop/src/components/ChatPanel.tsx#L245) · [Backend chạy agent](apps/desktop/src-tauri/src/commands.rs#L462)

## 5. Đọc, sửa và duyệt tài liệu

### Bảo toàn nội dung chưa lưu

Panel tải lại nội dung khi đổi file, feature hoặc nhận tín hiệu làm mới. Khi tải thành công, nội dung trong panel được gán lại và cờ `unsaved` bị xóa. Trong đường này chưa thấy cơ chế giữ nháp hoặc hỏi trước khi thay file. Kịch bản cần xác nhận là sửa một đoạn trong `spec.md`, bấm sang `plan.md`, rồi quay lại. Tôi xếp rủi ro này ở mức cao vì liên quan đến công sức người dùng. Nên giữ bản nháp theo đường dẫn hoặc yêu cầu chọn lưu, bỏ hay ở lại khi sắp rời tài liệu có chỉnh sửa. [Luồng tải và lưu tài liệu](apps/desktop/src/components/ArtifactPanel.tsx#L114)

### Duyệt nhiều checklist

Checkbox, số mục hoàn tất và thanh tiến độ giúp reviewer biết phải làm gì. Khi vào bước `Checklist`, panel tự chọn file đầu tiên. Thanh tab chỉ có `spec`, `plan`, `tasks`. Với hai checklist, chưa thấy đường đi trong panel để chọn file thứ hai, dù backend đếm các mục chưa duyệt từ tất cả checklist. Sau khi tick, panel tải lại file nhưng luồng này chưa gọi quét lại pipeline. Vì vậy số liệu trên panel và sidebar có thể lệch nhau đến lần quét tiếp theo. Nên hiện danh sách checklist kèm số mục còn lại, đồng bộ tiến độ sau khi lưu và cho mở từng checklist từ cảnh báo của `Implement`. [Panel tài liệu](apps/desktop/src/components/ArtifactPanel.tsx) · [Quét pipeline](apps/desktop/src/App.tsx#L31) · [Đếm mục chưa duyệt](apps/desktop/src-tauri/crates/speckit-core/src/state.rs)

### Đi từ kết quả đến thứ cần review

Khi backend ghi file, chat hiện `artifact written: <path>`. Các bước `Analyze`, `Implement` và `Converge` mặc định đưa cột phải về `tasks.md`. Điều này hữu ích để đối chiếu task, nhưng tôi vẫn phải tìm trong chat để biết phân tích phát hiện gì, agent đã đổi những file nào và còn việc gì sau `Converge`. Nên có tóm tắt ở cuối lần chạy, nút mở file hoặc kết quả liên quan và danh sách thay đổi cho bước triển khai. [Sự kiện ghi tài liệu](apps/desktop/src/components/ChatPanel.tsx#L286) · [Tài liệu mặc định theo bước](apps/desktop/src/components/ArtifactPanel.tsx#L22)

## 6. Đánh giá theo tiêu chí

| Tiêu chí | Nhận định |
|---|---|
| Phân cấp thị giác | **3/5.** Ba vùng dễ nhận ra. Hộp thoại kết nối provider chiếm điểm nhìn đầu tiên, còn hành động kế tiếp trong workspace chưa đủ nổi. |
| Nhất quán | **3,5/5.** Kiểu điều khiển cùng hệ. Cách viết tên bước ở sidebar và header khác nhau. Ý nghĩa của ổ khóa chưa bám sát điều kiện chạy thực tế. |
| Khả năng thao tác | **2,5/5.** Bước kế tiếp, phạm vi chạy, khôi phục sau lỗi và bảo toàn bản nháp cần được làm rõ. |
| Khả năng tiếp cận | **Chưa chấm.** Có nhãn cho nút icon và tay nắm resize trong code. Một số hàng pipeline dùng câu mô tả điều kiện làm `aria-label` thay cho tên bước, và trạng thái chủ yếu hiện qua icon. Chưa kiểm tra focus, screen reader và contrast. |
| Cửa sổ hẹp | **Chưa chấm.** Panel có thể resize. Ước lượng chiều rộng cho thấy composer, tài liệu và chat dễ cạnh tranh không gian ở kích thước tối thiểu. |
| Hiệu năng cảm nhận | **Chưa chấm.** Có spinner và streaming trong code. Chưa đo mở repo lớn, tài liệu dài hoặc lần chạy provider thật. |

## 7. Thứ tự cải thiện

1. **Bảo toàn công việc và làm rõ hành động ghi.** Giữ bản nháp khi đổi tab, giữ transcript theo lần chạy và nói rõ `Specify` tạo feature mới.
2. **Rút ngắn đường đến giá trị đầu tiên.** Mở repo trước, hiện feature và tài liệu nhận diện được, đề xuất bước kế tiếp rồi kết nối provider khi cần chạy.
3. **Hoàn thiện vòng chạy, đọc kết quả và duyệt.** Cho biết agent đang làm gì hoặc chờ ai, mở đúng file vừa ghi, truy cập từng checklist và hiện điều kiện còn thiếu.
4. **Chỉnh mật độ và kiểm tra cửa sổ hẹp.** Cho thu gọn panel tài liệu, gom bộ chọn trong composer, kiểm tra bàn phím, focus và độ tương phản trên các trạng thái thật.

## Phạm vi kiểm chứng

Đã quan sát màn hình khởi động trên Tauri ở 1440 × 900, đóng hộp thoại API key và mở trình chọn thư mục. Ảnh màn hình được lưu tại `/home/vietis/.agents/outputs/aix/artifacts/first-launch-tauri.png`. Trong lúc kiểm tra, Vite từng báo lỗi cú pháp sau một lần cập nhật `ArtifactPanel.tsx`. Lần thử lại đã mở được app và trình chọn thư mục, nhưng chưa hoàn tất chọn repo mẫu trong phiên Tauri cô lập. Các nhận xét về workspace đã mở repo, lần chạy, giữ nháp và checklist dựa trên mã nguồn. Chưa cấp API key hay chạy agent thật.
