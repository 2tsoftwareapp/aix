# Tasks — Điều hướng pipeline và lịch sử agent

## Mục tiêu

Khi đang xem một feature, người dùng có thể chuyển giữa các bước, đọc tài liệu và quay lại đúng lần chạy đang làm dở. Ba cột dùng chung project và feature, với trách nhiệm riêng:

| Cột | Trách nhiệm |
|---|---|
| Trái | Chọn project, feature và bước. Cho biết bước nào sẵn sàng, thiếu đầu vào hoặc cần duyệt. |
| Giữa | Hiện bản nháp, lịch sử chạy, kết quả và yêu cầu duyệt của bước đang xem. |
| Phải | Mở tài liệu mặc định theo bước. Cho xem, sửa và duyệt tài liệu của feature, kể cả khi đang xem một bước khác. |

Nguồn phân tích UX: [UI_UX_REVIEW.md](UI_UX_REVIEW.md).

## P0 — Tách bước đang xem khỏi lần chạy

- [ ] Khi bấm một bước ở sidebar, chỉ đổi bước đang xem. Giữ transcript và bản nháp theo project, feature và bước. Với bước cấp project như `Constitution`, dùng ngữ cảnh của project.
- [ ] Khi bấm `Run`, chốt repo, feature đích, bước, prompt và quyền tool của lần chạy. Gắn output, lỗi, yêu cầu duyệt và trạng thái kết thúc với đúng lần chạy, kể cả khi người dùng điều hướng sang nơi khác.
- [ ] Hiện dấu hiệu đang chạy hoặc chờ duyệt ở bước gốc. Cho quay về đúng lần chạy từ dấu hiệu đó. Nút `Stop` và thao tác duyệt phải tác động tới lần chạy được hiển thị.
- [ ] Giữ nội dung đầu vào để người dùng có thể khôi phục prompt khi lần chạy lỗi. Làm rõ rằng lịch sử hiển thị phục vụ việc xem lại. Khi chạy, agent nhận prompt đã gửi và tài liệu đã lưu theo backend hiện tại.

**Nghiệm thu**

1. Chạy `Clarify`, bấm `Plan` khi đang streaming rồi quay lại. Phản hồi tiếp tục xuất hiện trong `Clarify`, giữ cả nội dung trước khi chuyển bước.
2. Gõ nháp ở `Clarify`, chuyển sang `Plan`. Ô nhập hiển thị nháp riêng của `Plan`. Quay lại `Clarify` thấy nguyên bản nháp.
3. Đổi bước khi agent đang chờ duyệt. Quay lại từ chỉ báo chờ duyệt, trả lời và thấy lần chạy tiếp tục. Kết quả và tài liệu đầu ra thuộc đúng repo, feature và bước lúc gửi.
4. Chuyển sang feature khác có cùng tên bước. Chat và nháp của hai feature vẫn tách biệt.

Điểm vào: [App.tsx](apps/desktop/src/App.tsx), [ChatPanel.tsx](apps/desktop/src/components/ChatPanel.tsx), [commands.rs](apps/desktop/src-tauri/src/commands.rs).

## P0 — Cho đọc tài liệu độc lập và bảo toàn bản sửa

- [ ] Bấm tab tài liệu chỉ đổi file đang xem. Giữ bước đã chọn và lịch sử agent.
- [ ] Gắn nội dung editor và bản nháp với đúng repo, feature và đường dẫn. Xử lý đổi tab, đổi bước, đổi feature và tải lại sau khi agent ghi file khi còn bản sửa chưa lưu.
- [ ] Áp dụng kết quả đọc file đúng với yêu cầu còn hiệu lực. Trong lúc đổi file, mọi thao tác lưu phải trỏ đúng file sở hữu nội dung đang sửa.

**Nghiệm thu:** Đang ở `Plan`, mở `spec.md` để đối chiếu. Sửa spec, chuyển sang plan rồi quay lại. Bản sửa vẫn truy xuất được hoặc app đã hỏi cách xử lý trước khi chuyển. Dùng Ctrl+S khi file mới đang tải không ghi nội dung spec sang `plan.md`.

Điểm vào: [ArtifactPanel.tsx](apps/desktop/src/components/ArtifactPanel.tsx).

## P1 — Làm rõ trạng thái pipeline ở sidebar

- [ ] Hiển thị trạng thái theo dữ liệu của feature đang xem. Phân biệt đã có tài liệu, đã chạy và đã duyệt. Dùng chữ cạnh icon khi một trạng thái ảnh hưởng đến khả năng chạy.
- [ ] Khi một bước bị chặn, hiển thị điều kiện còn thiếu và lối sang bước hoặc tài liệu liên quan. Ví dụ `Implement · Thiếu tasks.md` dẫn sang `Tasks`.
- [ ] Làm rõ `Specify` tạo feature mới. Sau lần chạy thành công, cho mở đúng feature hoặc file vừa tạo từ kết quả của lần chạy.

**Nghiệm thu:** Mở feature đã có `spec.md` và chưa có `plan.md`. Sidebar thể hiện có spec, cho thấy `Plan` sẵn sàng và nêu lý do `Tasks` chưa chạy được. Việc có spec không tự đánh dấu `Clarify` đã hoàn tất.

Điểm vào: [Sidebar.tsx](apps/desktop/src/components/Sidebar.tsx), [gates.rs](apps/desktop/src-tauri/crates/speckit-core/src/gates.rs).

## Phạm vi

Các task trên tập trung vào điều hướng, lần chạy và tài liệu. Lịch sử qua lần đóng/mở app và việc tự gửi transcript cũ vào prompt là hai quyết định sản phẩm riêng. Tiêu chí trên yêu cầu giữ công việc khi điều hướng trong cùng phiên app.
